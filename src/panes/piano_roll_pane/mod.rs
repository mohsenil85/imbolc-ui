mod input;
mod rendering;

use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, InputEvent, Keymap, MouseEvent, Pane, PianoKeyboard, ToggleResult};

pub struct PianoRollPane {
    keymap: Keymap,
    // Cursor state
    pub(super) cursor_pitch: u8,   // MIDI note 0-127
    pub(super) cursor_tick: u32,   // Position in ticks
    // View state
    pub(super) current_track: usize,
    pub(super) view_bottom_pitch: u8,  // Lowest visible pitch
    pub(super) view_start_tick: u32,   // Leftmost visible tick
    pub(super) zoom_level: u8,         // 1=finest, higher=wider beats. Ticks per cell.
    // Note placement defaults
    pub(super) default_duration: u32,
    pub(super) default_velocity: u8,
    // Piano keyboard mode
    pub(super) piano: PianoKeyboard,
    pub(super) recording: bool,            // True when recording notes from piano keyboard
    // Automation overlay
    pub(super) automation_overlay_visible: bool,
    pub(super) automation_overlay_lane_idx: Option<usize>, // index into automation.lanes for overlay display
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
            automation_overlay_visible: false,
            automation_overlay_lane_idx: None,
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
    pub(crate) fn ticks_per_cell(&self) -> u32 {
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

    fn handle_action(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
        self.handle_action_impl(action, event, state)
    }

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action {
        self.handle_mouse_impl(event, area, state)
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_notes_buf(buf, area, &state.session.piano_roll);

        // Automation overlay
        if self.automation_overlay_visible {
            let rect = center_rect(area, 97, 29);
            let key_col_width: u16 = 5;
            let header_height: u16 = 2;
            let footer_height: u16 = 2;
            let grid_x = rect.x + key_col_width;
            let grid_width = rect.width.saturating_sub(key_col_width + 1);
            let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

            // Overlay occupies the bottom 4 rows of the grid area
            let overlay_rows = 4u16.min(grid_height / 2);
            let overlay_y = rect.y + header_height + grid_height - overlay_rows;
            let overlay_area = RatatuiRect::new(rect.x, overlay_y, rect.width, overlay_rows);

            self.render_automation_overlay(buf, overlay_area, grid_x, grid_width, state);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::ui::{InputEvent, KeyCode, Modifiers, PianoRollAction};

    fn dummy_event() -> InputEvent {
        InputEvent::new(KeyCode::Char('x'), Modifiers::default())
    }

    #[test]
    fn cursor_moves_with_arrow_actions() {
        let mut pane = PianoRollPane::new(Keymap::new());
        let state = AppState::new();

        let start_pitch = pane.cursor_pitch;
        pane.handle_action("up", &dummy_event(), &state);
        assert_eq!(pane.cursor_pitch, start_pitch + 1);

        pane.handle_action("down", &dummy_event(), &state);
        assert_eq!(pane.cursor_pitch, start_pitch);

        let start_tick = pane.cursor_tick;
        pane.handle_action("right", &dummy_event(), &state);
        assert!(pane.cursor_tick > start_tick);

        pane.handle_action("left", &dummy_event(), &state);
        assert_eq!(pane.cursor_tick, start_tick);
    }

    #[test]
    fn zoom_in_out_clamps() {
        let mut pane = PianoRollPane::new(Keymap::new());
        let state = AppState::new();

        pane.zoom_level = 1;
        pane.handle_action("zoom_in", &dummy_event(), &state);
        assert_eq!(pane.zoom_level, 1);

        pane.handle_action("zoom_out", &dummy_event(), &state);
        assert_eq!(pane.zoom_level, 2);
    }

    #[test]
    fn home_resets_cursor_and_view() {
        let mut pane = PianoRollPane::new(Keymap::new());
        let state = AppState::new();

        pane.cursor_tick = 960;
        pane.view_start_tick = 480;
        pane.handle_action("home", &dummy_event(), &state);
        assert_eq!(pane.cursor_tick, 0);
        assert_eq!(pane.view_start_tick, 0);
    }

    #[test]
    fn toggle_note_returns_action() {
        let mut pane = PianoRollPane::new(Keymap::new());
        let state = AppState::new();

        let action = pane.handle_action("toggle_note", &dummy_event(), &state);
        assert!(matches!(action, Action::PianoRoll(PianoRollAction::ToggleNote)));
    }
}
