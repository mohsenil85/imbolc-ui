use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::piano_roll::PianoRollState;
use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, MouseEvent, MouseEventKind, MouseButton, Pane, PianoKeyboard, PianoRollAction, Style, ToggleResult};

/// MIDI note name for a given pitch (0-127)
fn note_name(pitch: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (pitch / 12) as i8 - 1;
    let name = names[(pitch % 12) as usize];
    format!("{}{}", name, octave)
}

/// Check if a pitch is a black key
fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

pub struct PianoRollPane {
    keymap: Keymap,
    // Cursor state
    cursor_pitch: u8,   // MIDI note 0-127
    cursor_tick: u32,   // Position in ticks
    // View state
    current_track: usize,
    view_bottom_pitch: u8,  // Lowest visible pitch
    view_start_tick: u32,   // Leftmost visible tick
    zoom_level: u8,         // 1=finest, higher=wider beats. Ticks per cell.
    // Note placement defaults
    default_duration: u32,
    default_velocity: u8,
    // Piano keyboard mode
    piano: PianoKeyboard,
    recording: bool,            // True when recording notes from piano keyboard
}

impl PianoRollPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            cursor_pitch: 60, // C4
            cursor_tick: 0,
            current_track: 0,
            view_bottom_pitch: 48, // C3
            view_start_tick: 0,
            zoom_level: 3, // Each cell = 120 ticks (1/4 beat at 480 tpb)
            default_duration: 480, // One beat
            default_velocity: 100,
            piano: PianoKeyboard::new(),
            recording: false,
        }
    }

    // Accessors for main.rs
    pub fn cursor_pitch(&self) -> u8 { self.cursor_pitch }
    pub fn cursor_tick(&self) -> u32 { self.cursor_tick }
    pub fn default_duration(&self) -> u32 { self.default_duration }
    pub fn default_velocity(&self) -> u8 { self.default_velocity }
    pub fn current_track(&self) -> usize { self.current_track }
    pub fn is_recording(&self) -> bool { self.recording }
    pub fn set_recording(&mut self, recording: bool) { self.recording = recording; }

    pub fn adjust_default_duration(&mut self, delta: i32) {
        let new_dur = (self.default_duration as i32 + delta).max(self.ticks_per_cell() as i32);
        self.default_duration = new_dur as u32;
    }

    pub fn adjust_default_velocity(&mut self, delta: i8) {
        let new_vel = (self.default_velocity as i16 + delta as i16).clamp(1, 127);
        self.default_velocity = new_vel as u8;
    }

    pub fn change_track(&mut self, delta: i8, track_count: usize) {
        if track_count == 0 { return; }
        let new_idx = (self.current_track as i32 + delta as i32).clamp(0, track_count as i32 - 1);
        self.current_track = new_idx as usize;
    }

    /// Set current track index directly (for external syncing from global instrument selection)
    pub fn set_current_track(&mut self, idx: usize) {
        self.current_track = idx;
    }

    pub fn jump_to_end(&mut self) {
        // Jump to a reasonable far position (e.g., 16 bars worth)
        self.cursor_tick = 480 * 4 * 16; // 16 bars at 4/4
        self.scroll_to_cursor();
    }

    /// Ticks per grid cell based on zoom level
    fn ticks_per_cell(&self) -> u32 {
        match self.zoom_level {
            1 => 60,   // 1/8 beat
            2 => 120,  // 1/4 beat
            3 => 240,  // 1/2 beat
            4 => 480,  // 1 beat
            5 => 960,  // 2 beats
            _ => 240,
        }
    }

    /// Snap cursor tick to grid
    fn snap_tick(&self, tick: u32) -> u32 {
        let grid = self.ticks_per_cell();
        (tick / grid) * grid
    }

    /// Ensure cursor is visible by adjusting view
    fn scroll_to_cursor(&mut self) {
        // Vertical: keep cursor within visible range
        let visible_rows = 24u8;
        if self.cursor_pitch < self.view_bottom_pitch {
            self.view_bottom_pitch = self.cursor_pitch;
        } else if self.cursor_pitch >= self.view_bottom_pitch.saturating_add(visible_rows) {
            self.view_bottom_pitch = self.cursor_pitch.saturating_sub(visible_rows - 1);
        }

        // Horizontal: keep cursor within visible range
        let visible_cols = 60u32;
        let visible_ticks = visible_cols * self.ticks_per_cell();
        if self.cursor_tick < self.view_start_tick {
            self.view_start_tick = self.snap_tick(self.cursor_tick);
        } else if self.cursor_tick >= self.view_start_tick + visible_ticks {
            self.view_start_tick = self.snap_tick(self.cursor_tick.saturating_sub(visible_ticks - self.ticks_per_cell()));
        }
    }

    /// Center the view vertically on the current piano octave
    fn center_view_on_piano_octave(&mut self) {
        // Piano octave base note: octave 4 = C4 = MIDI 60
        let base_pitch = ((self.piano.octave() as i16 + 1) * 12).clamp(0, 127) as u8;
        // Center the view so the octave is roughly in the middle
        // visible_rows is about 24, so offset by ~12 to center
        let visible_rows = 24u8;
        self.view_bottom_pitch = base_pitch.saturating_sub(visible_rows / 2);
        // Also move cursor to the base note of this octave
        self.cursor_pitch = base_pitch;
    }

    /// Render notes grid (buffer version)
    fn render_notes_buf(&self, buf: &mut Buffer, area: RatatuiRect, piano_roll: &PianoRollState) {
        let rect = center_rect(area, 97, 29);

        // Layout constants
        let key_col_width: u16 = 5;
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + key_col_width;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(key_col_width + 1);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        // Border
        let track_label = if let Some(track) = piano_roll.track_at(self.current_track) {
            let mode = if track.polyphonic { "POLY" } else { "MONO" };
            format!(
                " Piano Roll: midi-{} [{}/{}] {} ",
                track.module_id,
                self.current_track + 1,
                piano_roll.track_order.len(),
                mode,
            )
        } else {
            " Piano Roll: (no tracks) ".to_string()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(track_label.as_str())
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::PINK)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::PINK)));
        block.render(rect, buf);

        // Header: transport info
        let header_y = rect.y + 1;
        let play_icon = if piano_roll.playing { "||" } else { "> " };
        let loop_icon = if piano_roll.looping { "L" } else { " " };
        let (ts_num, ts_den) = piano_roll.time_signature;
        let header_text = format!(
            " BPM:{:.0}  {}/{}  {}  {}  Beat:{:.1}",
            piano_roll.bpm, ts_num, ts_den, play_icon, loop_icon,
            piano_roll.tick_to_beat(piano_roll.playhead),
        );
        Paragraph::new(Line::from(Span::styled(
            header_text,
            ratatui::style::Style::from(Style::new().fg(Color::WHITE)),
        ))).render(RatatuiRect::new(rect.x + 1, header_y, rect.width.saturating_sub(2), 1), buf);

        // Loop range indicator
        if piano_roll.looping {
            let loop_info = format!(
                "Loop:{:.1}-{:.1}",
                piano_roll.tick_to_beat(piano_roll.loop_start),
                piano_roll.tick_to_beat(piano_roll.loop_end),
            );
            let loop_x = rect.x + rect.width - loop_info.len() as u16 - 2;
            Paragraph::new(Line::from(Span::styled(
                loop_info,
                ratatui::style::Style::from(Style::new().fg(Color::YELLOW)),
            ))).render(RatatuiRect::new(loop_x, header_y, rect.width.saturating_sub(loop_x - rect.x), 1), buf);
        }

        // Piano keys column + grid rows
        for row in 0..grid_height {
            let pitch = self.view_bottom_pitch.saturating_add((grid_height - 1 - row) as u8);
            if pitch > 127 {
                continue;
            }
            let y = grid_y + row;

            // Piano key label
            let name = note_name(pitch);
            let is_black = is_black_key(pitch);
            let key_style = if pitch == self.cursor_pitch {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
            } else if is_black {
                ratatui::style::Style::from(Style::new().fg(Color::GRAY))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE))
            };
            let key_str = format!("{:>3}", name);
            for (j, ch) in key_str.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((rect.x + 1 + j as u16, y)) {
                    cell.set_char(ch).set_style(key_style);
                }
            }

            // Separator
            let sep_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
            if let Some(cell) = buf.cell_mut((rect.x + key_col_width - 1, y)) {
                cell.set_char('|').set_style(sep_style);
            }

            // Grid cells
            for col in 0..grid_width {
                let tick = self.view_start_tick + col as u32 * self.ticks_per_cell();
                let x = grid_x + col;

                let has_note = piano_roll.track_at(self.current_track).map_or(false, |track| {
                    track.notes.iter().any(|n| {
                        n.pitch == pitch && tick >= n.tick && tick < n.tick + n.duration
                    })
                });

                let is_note_start = piano_roll.track_at(self.current_track).map_or(false, |track| {
                    track.notes.iter().any(|n| n.pitch == pitch && n.tick == tick)
                });

                let is_cursor = pitch == self.cursor_pitch && tick == self.cursor_tick;
                let is_playhead = piano_roll.playing
                    && tick <= piano_roll.playhead
                    && piano_roll.playhead < tick + self.ticks_per_cell();

                let tpb = piano_roll.ticks_per_beat;
                let tpbar = piano_roll.ticks_per_bar();
                let is_bar_line = tick % tpbar == 0;
                let is_beat_line = tick % tpb == 0;

                let (ch, style) = if is_cursor {
                    if has_note {
                        ('█', ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::WHITE)))
                    } else {
                        ('▒', ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)))
                    }
                } else if has_note {
                    if is_note_start {
                        ('█', ratatui::style::Style::from(Style::new().fg(Color::PINK)))
                    } else {
                        ('█', ratatui::style::Style::from(Style::new().fg(Color::MAGENTA)))
                    }
                } else if is_playhead {
                    ('│', ratatui::style::Style::from(Style::new().fg(Color::GREEN)))
                } else if is_bar_line {
                    ('┊', ratatui::style::Style::from(Style::new().fg(Color::GRAY)))
                } else if is_beat_line {
                    ('·', ratatui::style::Style::from(Style::new().fg(Color::new(40, 40, 40))))
                } else if is_black {
                    ('·', ratatui::style::Style::from(Style::new().fg(Color::new(25, 25, 25))))
                } else {
                    (' ', ratatui::style::Style::default())
                };

                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch).set_style(style);
                }
            }
        }

        // Footer: beat markers
        let footer_y = grid_y + grid_height;
        for col in 0..grid_width {
            let tick = self.view_start_tick + col as u32 * self.ticks_per_cell();
            let tpb = piano_roll.ticks_per_beat;
            let tpbar = piano_roll.ticks_per_bar();
            let x = grid_x + col;

            if tick % tpbar == 0 {
                let bar = tick / tpbar + 1;
                let label = format!("{}", bar);
                let white = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
                for (j, ch) in label.chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((x + j as u16, footer_y)) {
                        cell.set_char(ch).set_style(white);
                    }
                }
            } else if tick % tpb == 0 {
                let gray = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
                if let Some(cell) = buf.cell_mut((x, footer_y)) {
                    cell.set_char('·').set_style(gray);
                }
            }
        }

        // Status line
        let status_y = footer_y + 1;
        let vel_str = format!(
            "Note:{} Tick:{} Vel:{} Dur:{}",
            note_name(self.cursor_pitch),
            self.cursor_tick,
            self.default_velocity,
            self.default_duration,
        );
        Paragraph::new(Line::from(Span::styled(
            vel_str,
            ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
        ))).render(RatatuiRect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);

        // Piano mode indicator
        if self.piano.is_active() {
            let piano_str = self.piano.status_label();
            let mut indicator_x = rect.x + rect.width - piano_str.len() as u16 - 1;

            if self.recording {
                let rec_str = " REC ";
                indicator_x -= rec_str.len() as u16;
                let rec_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::RED));
                for (j, ch) in rec_str.chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((indicator_x + j as u16, status_y)) {
                        cell.set_char(ch).set_style(rec_style);
                    }
                }
                indicator_x += rec_str.len() as u16;
            }

            let piano_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::PINK));
            for (j, ch) in piano_str.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((indicator_x + j as u16, status_y)) {
                    cell.set_char(ch).set_style(piano_style);
                }
            }
        } else {
            let hint_str = "/=piano";
            let hint_x = rect.x + rect.width - hint_str.len() as u16 - 2;
            Paragraph::new(Line::from(Span::styled(
                hint_str,
                ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
            ))).render(RatatuiRect::new(hint_x, status_y, hint_str.len() as u16, 1), buf);
        }
    }
}

impl Default for PianoRollPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for PianoRollPane {
    fn id(&self) -> &'static str {
        "piano_roll"
    }

    fn handle_action(&mut self, action: &str, event: &InputEvent, _state: &AppState) -> Action {
        match action {
            // Piano mode actions (from piano layer)
            "piano:escape" => {
                let was_active = self.piano.is_active();
                self.piano.handle_escape();
                if was_active && !self.piano.is_active() {
                    Action::ExitPerformanceMode
                } else {
                    Action::None
                }
            }
            "piano:octave_down" => {
                if self.piano.octave_down() {
                    self.center_view_on_piano_octave();
                }
                Action::None
            }
            "piano:octave_up" => {
                if self.piano.octave_up() {
                    self.center_view_on_piano_octave();
                }
                Action::None
            }
            "piano:space" => Action::PianoRoll(PianoRollAction::PlayStopRecord),
            "piano:key" => {
                if let KeyCode::Char(c) = event.key {
                    if let Some(pitches) = self.piano.key_to_pitches(c) {
                        if pitches.len() == 1 {
                            return Action::PianoRoll(PianoRollAction::PlayNote(pitches[0], 100));
                        } else {
                            return Action::PianoRoll(PianoRollAction::PlayNotes(pitches, 100));
                        }
                    }
                }
                Action::None
            }
            // Normal grid navigation
            "up" => {
                if self.cursor_pitch < 127 {
                    self.cursor_pitch += 1;
                    self.scroll_to_cursor();
                }
                Action::None
            }
            "down" => {
                if self.cursor_pitch > 0 {
                    self.cursor_pitch -= 1;
                    self.scroll_to_cursor();
                }
                Action::None
            }
            "right" => {
                self.cursor_tick += self.ticks_per_cell();
                self.scroll_to_cursor();
                Action::None
            }
            "left" => {
                let step = self.ticks_per_cell();
                self.cursor_tick = self.cursor_tick.saturating_sub(step);
                self.scroll_to_cursor();
                Action::None
            }
            "toggle_note" => Action::PianoRoll(PianoRollAction::ToggleNote),
            "grow_duration" => Action::PianoRoll(PianoRollAction::AdjustDuration(self.ticks_per_cell() as i32)),
            "shrink_duration" => Action::PianoRoll(PianoRollAction::AdjustDuration(-(self.ticks_per_cell() as i32))),
            "vel_up" => Action::PianoRoll(PianoRollAction::AdjustVelocity(10)),
            "vel_down" => Action::PianoRoll(PianoRollAction::AdjustVelocity(-10)),
            "play_stop" => Action::PianoRoll(PianoRollAction::PlayStop),
            "loop" => Action::PianoRoll(PianoRollAction::ToggleLoop),
            "loop_start" => Action::PianoRoll(PianoRollAction::SetLoopStart),
            "loop_end" => Action::PianoRoll(PianoRollAction::SetLoopEnd),
            "octave_up" => {
                self.cursor_pitch = (self.cursor_pitch as i16 + 12).min(127) as u8;
                self.scroll_to_cursor();
                Action::None
            }
            "octave_down" => {
                self.cursor_pitch = (self.cursor_pitch as i16 - 12).max(0) as u8;
                self.scroll_to_cursor();
                Action::None
            }
            "home" => {
                self.cursor_tick = 0;
                self.view_start_tick = 0;
                Action::None
            }
            "end" => Action::PianoRoll(PianoRollAction::Jump(1)),
            "zoom_in" => {
                if self.zoom_level > 1 {
                    self.zoom_level -= 1;
                    self.cursor_tick = self.snap_tick(self.cursor_tick);
                    self.scroll_to_cursor();
                }
                Action::None
            }
            "zoom_out" => {
                if self.zoom_level < 5 {
                    self.zoom_level += 1;
                    self.cursor_tick = self.snap_tick(self.cursor_tick);
                    self.scroll_to_cursor();
                }
                Action::None
            }
            "time_sig" => Action::PianoRoll(PianoRollAction::CycleTimeSig),
            "toggle_poly" => Action::PianoRoll(PianoRollAction::TogglePolyMode),
            _ => Action::None,
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, _state: &AppState) -> Action {
        let rect = center_rect(area, 97, 29);
        let key_col_width: u16 = 5;
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + key_col_width;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(key_col_width + 1);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        let col = event.column;
        let row = event.row;

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Click on the grid area
                if col >= grid_x && col < grid_x + grid_width
                    && row >= grid_y && row < grid_y + grid_height
                {
                    let grid_col = col - grid_x;
                    let grid_row = row - grid_y;
                    let pitch = self.view_bottom_pitch.saturating_add((grid_height - 1 - grid_row) as u8);
                    let tick = self.view_start_tick + grid_col as u32 * self.ticks_per_cell();

                    if pitch <= 127 {
                        self.cursor_pitch = pitch;
                        self.cursor_tick = tick;
                        return Action::PianoRoll(PianoRollAction::ToggleNote);
                    }
                }
                // Click on piano key column to set pitch
                if col >= rect.x && col < grid_x && row >= grid_y && row < grid_y + grid_height {
                    let grid_row = row - grid_y;
                    let pitch = self.view_bottom_pitch.saturating_add((grid_height - 1 - grid_row) as u8);
                    if pitch <= 127 {
                        self.cursor_pitch = pitch;
                    }
                }
                Action::None
            }
            MouseEventKind::Down(MouseButton::Right) => {
                // Right-click on grid: just move cursor (no toggle)
                if col >= grid_x && col < grid_x + grid_width
                    && row >= grid_y && row < grid_y + grid_height
                {
                    let grid_col = col - grid_x;
                    let grid_row = row - grid_y;
                    let pitch = self.view_bottom_pitch.saturating_add((grid_height - 1 - grid_row) as u8);
                    let tick = self.view_start_tick + grid_col as u32 * self.ticks_per_cell();
                    if pitch <= 127 {
                        self.cursor_pitch = pitch;
                        self.cursor_tick = tick;
                    }
                }
                Action::None
            }
            MouseEventKind::ScrollUp => {
                if event.modifiers.shift {
                    // Horizontal scroll
                    let step = self.ticks_per_cell() * 4;
                    self.view_start_tick = self.view_start_tick.saturating_sub(step);
                } else {
                    // Vertical scroll - pitch up
                    self.view_bottom_pitch = self.view_bottom_pitch.saturating_add(3).min(127);
                }
                Action::None
            }
            MouseEventKind::ScrollDown => {
                if event.modifiers.shift {
                    // Horizontal scroll
                    let step = self.ticks_per_cell() * 4;
                    self.view_start_tick += step;
                } else {
                    // Vertical scroll - pitch down
                    self.view_bottom_pitch = self.view_bottom_pitch.saturating_sub(3);
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_notes_buf(buf, area, &state.session.piano_roll);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn toggle_performance_mode(&mut self, _state: &AppState) -> ToggleResult {
        if self.piano.is_active() {
            self.piano.handle_escape();
            if self.piano.is_active() {
                ToggleResult::CycledLayout
            } else {
                ToggleResult::Deactivated
            }
        } else {
            self.piano.activate();
            ToggleResult::ActivatedPiano
        }
    }

    fn activate_piano(&mut self) {
        if !self.piano.is_active() { self.piano.activate(); }
    }

    fn deactivate_performance(&mut self) {
        self.piano.deactivate();
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
