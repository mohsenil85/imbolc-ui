use std::any::Any;

use crate::state::piano_roll::PianoRollState;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

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
}

impl PianoRollPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Up, "up", "Cursor up (higher pitch)")
                .bind_key(KeyCode::Down, "down", "Cursor down (lower pitch)")
                .bind_key(KeyCode::Left, "left", "Cursor left (earlier)")
                .bind_key(KeyCode::Right, "right", "Cursor later (later)")
                .bind_key(KeyCode::Enter, "toggle_note", "Place/remove note")
                // Shift+Left/Right handled manually in handle_input
                .bind('+', "vel_up", "Increase velocity")
                .bind('-', "vel_down", "Decrease velocity")
                .bind(' ', "play_stop", "Play / Stop")
                .bind('l', "loop", "Toggle loop")
                .bind('[', "loop_start", "Set loop start")
                .bind(']', "loop_end", "Set loop end")
                .bind('<', "prev_track", "Previous track")
                .bind('>', "next_track", "Next track")
                .bind_key(KeyCode::PageUp, "octave_up", "Scroll up one octave")
                .bind_key(KeyCode::PageDown, "octave_down", "Scroll down one octave")
                .bind_key(KeyCode::Home, "home", "Jump to start")
                .bind_key(KeyCode::End, "end", "Jump to end")
                .bind('z', "zoom_in", "Zoom in (time)")
                .bind('x', "zoom_out", "Zoom out (time)")
                .bind('t', "time_sig", "Cycle time signature"),
            cursor_pitch: 60, // C4
            cursor_tick: 0,
            current_track: 0,
            view_bottom_pitch: 48, // C3
            view_start_tick: 0,
            zoom_level: 3, // Each cell = 120 ticks (1/4 beat at 480 tpb)
            default_duration: 480, // One beat
            default_velocity: 100,
        }
    }

    // Accessors for main.rs
    pub fn cursor_pitch(&self) -> u8 { self.cursor_pitch }
    pub fn cursor_tick(&self) -> u32 { self.cursor_tick }
    pub fn default_duration(&self) -> u32 { self.default_duration }
    pub fn default_velocity(&self) -> u8 { self.default_velocity }
    pub fn current_track(&self) -> usize { self.current_track }

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

    /// Render with external piano roll state
    pub fn render_with_state(&self, g: &mut dyn Graphics, piano_roll: &PianoRollState) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        // Layout constants
        let key_col_width: u16 = 5;
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + key_col_width;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(key_col_width + 1);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        // Border
        g.set_style(Style::new().fg(Color::PINK));
        let track_label = if let Some(track) = piano_roll.track_at(self.current_track) {
            format!(
                " Piano Roll: midi-{} [{}/{}] ",
                track.module_id,
                self.current_track + 1,
                piano_roll.track_order.len()
            )
        } else {
            " Piano Roll: (no tracks) ".to_string()
        };
        g.draw_box(rect, Some(&track_label));

        // Header: transport info
        let header_y = rect.y + 1;
        g.set_style(Style::new().fg(Color::WHITE));
        let play_icon = if piano_roll.playing { "||" } else { "> " };
        let loop_icon = if piano_roll.looping { "L" } else { " " };
        let (ts_num, ts_den) = piano_roll.time_signature;
        let header_text = format!(
            " BPM:{:.0}  {}/{}  {}  {}  Beat:{:.1}",
            piano_roll.bpm,
            ts_num,
            ts_den,
            play_icon,
            loop_icon,
            piano_roll.tick_to_beat(piano_roll.playhead),
        );
        g.put_str(rect.x + 1, header_y, &header_text);

        // Loop range indicator
        if piano_roll.looping {
            let loop_info = format!(
                "Loop:{:.1}-{:.1}",
                piano_roll.tick_to_beat(piano_roll.loop_start),
                piano_roll.tick_to_beat(piano_roll.loop_end),
            );
            g.set_style(Style::new().fg(Color::YELLOW));
            g.put_str(rect.x + rect.width - loop_info.len() as u16 - 2, header_y, &loop_info);
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
            if pitch == self.cursor_pitch {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
            } else if is_black {
                g.set_style(Style::new().fg(Color::GRAY));
            } else {
                g.set_style(Style::new().fg(Color::WHITE));
            }
            g.put_str(rect.x + 1, y, &format!("{:>3}", name));

            // Separator
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_char(rect.x + key_col_width - 1, y, '|');

            // Grid cells
            for col in 0..grid_width {
                let tick = self.view_start_tick + col as u32 * self.ticks_per_cell();
                let x = grid_x + col;

                // Check if there's a note here
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

                // Beat/bar grid lines
                let tpb = piano_roll.ticks_per_beat;
                let tpbar = piano_roll.ticks_per_bar();
                let is_bar_line = tick % tpbar == 0;
                let is_beat_line = tick % tpb == 0;

                if is_cursor {
                    if has_note {
                        g.set_style(Style::new().fg(Color::BLACK).bg(Color::WHITE));
                        g.put_char(x, y, '█');
                    } else {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                        g.put_char(x, y, '▒');
                    }
                } else if has_note {
                    if is_note_start {
                        g.set_style(Style::new().fg(Color::PINK));
                    } else {
                        g.set_style(Style::new().fg(Color::MAGENTA));
                    }
                    g.put_char(x, y, '█');
                } else if is_playhead {
                    g.set_style(Style::new().fg(Color::GREEN));
                    g.put_char(x, y, '│');
                } else if is_bar_line {
                    g.set_style(Style::new().fg(Color::GRAY));
                    g.put_char(x, y, '┊');
                } else if is_beat_line {
                    g.set_style(Style::new().fg(Color::new(40, 40, 40)));
                    g.put_char(x, y, '·');
                } else if is_black {
                    g.set_style(Style::new().fg(Color::new(25, 25, 25)));
                    g.put_char(x, y, '·');
                } else {
                    g.put_char(x, y, ' ');
                }
            }
        }

        // Footer: beat markers
        let footer_y = grid_y + grid_height;
        g.set_style(Style::new().fg(Color::GRAY));
        for col in 0..grid_width {
            let tick = self.view_start_tick + col as u32 * self.ticks_per_cell();
            let tpb = piano_roll.ticks_per_beat;
            let tpbar = piano_roll.ticks_per_bar();
            let x = grid_x + col;

            if tick % tpbar == 0 {
                let bar = tick / tpbar + 1;
                let label = format!("{}", bar);
                g.set_style(Style::new().fg(Color::WHITE));
                g.put_str(x, footer_y, &label);
            } else if tick % tpb == 0 {
                g.set_style(Style::new().fg(Color::GRAY));
                g.put_char(x, footer_y, '·');
            }
        }

        // Status line
        let status_y = footer_y + 1;
        g.set_style(Style::new().fg(Color::GRAY));
        let vel_str = format!(
            "Note:{} Tick:{} Vel:{} Dur:{}",
            note_name(self.cursor_pitch),
            self.cursor_tick,
            self.default_velocity,
            self.default_duration,
        );
        g.put_str(rect.x + 1, status_y, &vel_str);
    }
}

impl Default for PianoRollPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for PianoRollPane {
    fn id(&self) -> &'static str {
        "piano_roll"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        // Handle shift+arrow for duration adjustment (before keymap, since keymap doesn't support shift)
        if event.modifiers.shift {
            match event.key {
                KeyCode::Right => return Action::PianoRollAdjustDuration(self.ticks_per_cell() as i32),
                KeyCode::Left => return Action::PianoRollAdjustDuration(-(self.ticks_per_cell() as i32)),
                _ => {}
            }
        }

        match self.keymap.lookup(&event) {
            Some("up") => {
                if self.cursor_pitch < 127 {
                    self.cursor_pitch += 1;
                    self.scroll_to_cursor();
                }
                Action::None
            }
            Some("down") => {
                if self.cursor_pitch > 0 {
                    self.cursor_pitch -= 1;
                    self.scroll_to_cursor();
                }
                Action::None
            }
            Some("right") => {
                self.cursor_tick += self.ticks_per_cell();
                self.scroll_to_cursor();
                Action::None
            }
            Some("left") => {
                let step = self.ticks_per_cell();
                self.cursor_tick = self.cursor_tick.saturating_sub(step);
                self.scroll_to_cursor();
                Action::None
            }
            Some("toggle_note") => Action::PianoRollToggleNote,
            Some("grow") => Action::PianoRollAdjustDuration(self.ticks_per_cell() as i32),
            Some("shrink") => Action::PianoRollAdjustDuration(-(self.ticks_per_cell() as i32)),
            Some("vel_up") => Action::PianoRollAdjustVelocity(10),
            Some("vel_down") => Action::PianoRollAdjustVelocity(-10),
            Some("play_stop") => Action::PianoRollPlayStop,
            Some("loop") => Action::PianoRollToggleLoop,
            Some("loop_start") => Action::PianoRollSetLoopStart,
            Some("loop_end") => Action::PianoRollSetLoopEnd,
            Some("prev_track") => Action::PianoRollChangeTrack(-1),
            Some("next_track") => Action::PianoRollChangeTrack(1),
            Some("octave_up") => {
                self.cursor_pitch = (self.cursor_pitch as i16 + 12).min(127) as u8;
                self.scroll_to_cursor();
                Action::None
            }
            Some("octave_down") => {
                self.cursor_pitch = (self.cursor_pitch as i16 - 12).max(0) as u8;
                self.scroll_to_cursor();
                Action::None
            }
            Some("home") => {
                self.cursor_tick = 0;
                self.view_start_tick = 0;
                Action::None
            }
            Some("end") => Action::PianoRollJump(1),
            Some("zoom_in") => {
                if self.zoom_level > 1 {
                    self.zoom_level -= 1;
                    self.cursor_tick = self.snap_tick(self.cursor_tick);
                    self.scroll_to_cursor();
                }
                Action::None
            }
            Some("zoom_out") => {
                if self.zoom_level < 5 {
                    self.zoom_level += 1;
                    self.cursor_tick = self.snap_tick(self.cursor_tick);
                    self.scroll_to_cursor();
                }
                Action::None
            }
            Some("time_sig") => Action::PianoRollCycleTimeSig,
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        // Fallback render with empty state (actual rendering uses render_with_state)
        let empty = PianoRollState::new();
        self.render_with_state(g, &empty);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
