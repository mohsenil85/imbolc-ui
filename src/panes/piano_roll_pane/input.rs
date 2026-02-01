use ratatui::layout::Rect as RatatuiRect;

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, InputEvent, KeyCode, MouseButton, MouseEvent, MouseEventKind, PianoRollAction, translate_key};

use super::PianoRollPane;

impl PianoRollPane {
    pub(super) fn handle_action_impl(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
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
                    let c = translate_key(c, state.keyboard_layout);
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
            "toggle_automation" => {
                self.automation_overlay_visible = !self.automation_overlay_visible;
                Action::None
            }
            "automation_lane_prev" => {
                if self.automation_overlay_visible {
                    match self.automation_overlay_lane_idx {
                        Some(idx) if idx > 0 => {
                            self.automation_overlay_lane_idx = Some(idx - 1);
                        }
                        _ => {}
                    }
                }
                Action::None
            }
            "automation_lane_next" => {
                if self.automation_overlay_visible {
                    let next = match self.automation_overlay_lane_idx {
                        Some(idx) => idx + 1,
                        None => 1,
                    };
                    // Will be clamped during render based on actual lane count
                    self.automation_overlay_lane_idx = Some(next);
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    pub(super) fn handle_mouse_impl(&mut self, event: &MouseEvent, area: RatatuiRect, _state: &AppState) -> Action {
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
}
