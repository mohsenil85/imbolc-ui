use std::any::Any;

use crate::state::music::{Key, Scale};
use crate::state::AppState;
use crate::ui::frame::SessionState;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};
use crate::ui::widgets::TextInput;

/// Fields editable in the frame editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Bpm,
    TimeSig,
    Tuning,
    Key,
    Scale,
    Snap,
}

const FIELDS: [Field; 6] = [Field::Bpm, Field::TimeSig, Field::Tuning, Field::Key, Field::Scale, Field::Snap];

pub struct FrameEditPane {
    keymap: Keymap,
    session: SessionState,
    selected: usize,
    editing: bool,
    edit_input: TextInput,
}

impl FrameEditPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Up, "prev", "Previous field")
                .bind_key(KeyCode::Down, "next", "Next field")
                .bind_key(KeyCode::Left, "decrease", "Decrease value")
                .bind_key(KeyCode::Right, "increase", "Increase value")
                .bind_key(KeyCode::Enter, "confirm", "Confirm changes")
                .bind_key(KeyCode::Escape, "cancel", "Cancel"),
            session: SessionState::default(),
            selected: 0,
            editing: false,
            edit_input: TextInput::new(""),
        }
    }

    /// Set session state to edit (called before switching to this pane)
    pub fn set_session(&mut self, session: SessionState) {
        self.session = session;
        self.selected = 0;
        self.editing = false;
    }

    fn current_field(&self) -> Field {
        FIELDS[self.selected]
    }

    fn cycle_key(&mut self, forward: bool) {
        let idx = Key::ALL.iter().position(|k| *k == self.session.key).unwrap_or(0);
        self.session.key = if forward {
            Key::ALL[(idx + 1) % 12]
        } else {
            Key::ALL[(idx + 11) % 12]
        };
    }

    fn cycle_scale(&mut self, forward: bool) {
        let idx = Scale::ALL.iter().position(|s| *s == self.session.scale).unwrap_or(0);
        let len = Scale::ALL.len();
        self.session.scale = if forward {
            Scale::ALL[(idx + 1) % len]
        } else {
            Scale::ALL[(idx + len - 1) % len]
        };
    }

    const TIME_SIGS: [(u8, u8); 5] = [(4, 4), (3, 4), (6, 8), (5, 4), (7, 8)];

    fn cycle_time_sig(&mut self, forward: bool) {
        let idx = Self::TIME_SIGS.iter().position(|ts| *ts == self.session.time_signature).unwrap_or(0);
        let len = Self::TIME_SIGS.len();
        self.session.time_signature = if forward {
            Self::TIME_SIGS[(idx + 1) % len]
        } else {
            Self::TIME_SIGS[(idx + len - 1) % len]
        };
    }

    fn adjust(&mut self, increase: bool) {
        match self.current_field() {
            Field::Bpm => {
                let delta: i16 = if increase { 1 } else { -1 };
                self.session.bpm = (self.session.bpm as i16 + delta).clamp(20, 300) as u16;
            }
            Field::TimeSig => self.cycle_time_sig(increase),
            Field::Tuning => {
                let delta: f32 = if increase { 1.0 } else { -1.0 };
                self.session.tuning_a4 = (self.session.tuning_a4 + delta).clamp(400.0, 480.0);
            }
            Field::Key => self.cycle_key(increase),
            Field::Scale => self.cycle_scale(increase),
            Field::Snap => self.session.snap = !self.session.snap,
        }
    }

    fn field_label(field: Field) -> &'static str {
        match field {
            Field::Bpm => "BPM",
            Field::TimeSig => "Time Sig",
            Field::Tuning => "Tuning (A4)",
            Field::Key => "Key",
            Field::Scale => "Scale",
            Field::Snap => "Snap",
        }
    }

    fn field_value(&self, field: Field) -> String {
        match field {
            Field::Bpm => format!("{}", self.session.bpm),
            Field::TimeSig => format!("{}/{}", self.session.time_signature.0, self.session.time_signature.1),
            Field::Tuning => format!("{:.1} Hz", self.session.tuning_a4),
            Field::Key => self.session.key.name().to_string(),
            Field::Scale => self.session.scale.name().to_string(),
            Field::Snap => if self.session.snap { "ON".into() } else { "OFF".into() },
        }
    }
}

impl Default for FrameEditPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for FrameEditPane {
    fn id(&self) -> &'static str {
        "frame_edit"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        // Text editing mode for BPM/Tuning
        if self.editing {
            match event.key {
                KeyCode::Enter => {
                    let text = self.edit_input.value().to_string();
                    match self.current_field() {
                        Field::Bpm => {
                            if let Ok(v) = text.parse::<u16>() {
                                self.session.bpm = v.clamp(20, 300);
                            }
                        }
                        Field::Tuning => {
                            if let Ok(v) = text.parse::<f32>() {
                                self.session.tuning_a4 = v.clamp(400.0, 480.0);
                            }
                        }
                        _ => {}
                    }
                    self.editing = false;
                    self.edit_input.set_focused(false);
                    return Action::None;
                }
                KeyCode::Escape => {
                    self.editing = false;
                    self.edit_input.set_focused(false);
                    return Action::None;
                }
                _ => {
                    self.edit_input.handle_input(&event);
                    return Action::None;
                }
            }
        }

        match self.keymap.lookup(&event) {
            Some("prev") => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            Some("next") => {
                if self.selected < FIELDS.len() - 1 {
                    self.selected += 1;
                }
                Action::None
            }
            Some("decrease") => {
                self.adjust(false);
                Action::None
            }
            Some("increase") => {
                self.adjust(true);
                Action::None
            }
            Some("confirm") => {
                // For numeric fields, enter text editing; for others, confirm
                let field = self.current_field();
                if matches!(field, Field::Bpm | Field::Tuning) {
                    let val = match field {
                        Field::Bpm => format!("{}", self.session.bpm),
                        Field::Tuning => format!("{:.1}", self.session.tuning_a4),
                        _ => unreachable!(),
                    };
                    self.edit_input.set_value(&val);
                    self.edit_input.set_focused(true);
                    self.editing = true;
                    Action::None
                } else {
                    Action::UpdateSession(self.session.clone())
                }
            }
            Some("cancel") => {
                Action::SwitchPane("rack")
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, _state: &AppState) {
        let (width, height) = g.size();
        let box_width = 50;
        let box_height = 13;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::CYAN));
        g.draw_box(rect, Some(" Session "));

        let content_x = rect.x + 3;
        let list_y = rect.y + 2;

        for (i, field) in FIELDS.iter().enumerate() {
            let y = list_y + i as u16;
            let is_selected = i == self.selected;

            // Indicator
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x - 1, y, ">");
            }

            // Label
            let label_style = if is_selected {
                Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG)
            } else {
                Style::new().fg(Color::CYAN)
            };
            g.set_style(label_style);
            g.put_str(content_x + 1, y, &format!("{:14}", Self::field_label(*field)));

            // Value (or text input)
            if is_selected && self.editing {
                self.edit_input.render(g, content_x + 16, y, 16);
            } else {
                let val_style = if is_selected {
                    Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)
                } else {
                    Style::new().fg(Color::WHITE)
                };
                g.set_style(val_style);
                let val = self.field_value(*field);
                g.put_str(content_x + 16, y, &val);
            }

            // Clear rest of line if selected
            if is_selected {
                let val_len = if self.editing { 16 } else { self.field_value(*field).len() };
                let line_end = content_x + 16 + val_len as u16;
                g.set_style(Style::new().bg(Color::SELECTION_BG));
                for x in line_end..(rect.x + rect.width - 2) {
                    g.put_char(x, y, ' ');
                }
            }
        }

        // Help
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        let help = if self.editing {
            "Enter: confirm | Esc: cancel"
        } else {
            "Left/Right: adjust | Enter: type/confirm | Esc: cancel"
        };
        g.put_str(content_x, help_y, help);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
