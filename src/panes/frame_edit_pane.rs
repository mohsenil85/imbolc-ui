use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::music::{Key, Scale};
use crate::state::{AppState, MusicalSettings};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, NavAction, Pane, SessionAction, Style};
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
    settings: MusicalSettings,
    selected: usize,
    editing: bool,
    edit_input: TextInput,
}

impl FrameEditPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            settings: MusicalSettings::default(),
            selected: 0,
            editing: false,
            edit_input: TextInput::new(""),
        }
    }

    /// Set musical settings to edit (called before switching to this pane)
    #[allow(dead_code)]
    pub fn set_settings(&mut self, settings: MusicalSettings) {
        self.settings = settings;
        self.selected = 0;
        self.editing = false;
    }

    fn current_field(&self) -> Field {
        FIELDS[self.selected]
    }

    fn cycle_key(&mut self, forward: bool) {
        let idx = Key::ALL.iter().position(|k| *k == self.settings.key).unwrap_or(0);
        self.settings.key = if forward {
            Key::ALL[(idx + 1) % 12]
        } else {
            Key::ALL[(idx + 11) % 12]
        };
    }

    fn cycle_scale(&mut self, forward: bool) {
        let idx = Scale::ALL.iter().position(|s| *s == self.settings.scale).unwrap_or(0);
        let len = Scale::ALL.len();
        self.settings.scale = if forward {
            Scale::ALL[(idx + 1) % len]
        } else {
            Scale::ALL[(idx + len - 1) % len]
        };
    }

    const TIME_SIGS: [(u8, u8); 5] = [(4, 4), (3, 4), (6, 8), (5, 4), (7, 8)];

    fn cycle_time_sig(&mut self, forward: bool) {
        let idx = Self::TIME_SIGS.iter().position(|ts| *ts == self.settings.time_signature).unwrap_or(0);
        let len = Self::TIME_SIGS.len();
        self.settings.time_signature = if forward {
            Self::TIME_SIGS[(idx + 1) % len]
        } else {
            Self::TIME_SIGS[(idx + len - 1) % len]
        };
    }

    fn adjust(&mut self, increase: bool) {
        match self.current_field() {
            Field::Bpm => {
                let delta: i16 = if increase { 1 } else { -1 };
                self.settings.bpm = (self.settings.bpm as i16 + delta).clamp(20, 300) as u16;
            }
            Field::TimeSig => self.cycle_time_sig(increase),
            Field::Tuning => {
                let delta: f32 = if increase { 1.0 } else { -1.0 };
                self.settings.tuning_a4 = (self.settings.tuning_a4 + delta).clamp(400.0, 480.0);
            }
            Field::Key => self.cycle_key(increase),
            Field::Scale => self.cycle_scale(increase),
            Field::Snap => self.settings.snap = !self.settings.snap,
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
            Field::Bpm => format!("{}", self.settings.bpm),
            Field::TimeSig => format!("{}/{}", self.settings.time_signature.0, self.settings.time_signature.1),
            Field::Tuning => format!("{:.1} Hz", self.settings.tuning_a4),
            Field::Key => self.settings.key.name().to_string(),
            Field::Scale => self.settings.scale.name().to_string(),
            Field::Snap => if self.settings.snap { "ON".into() } else { "OFF".into() },
        }
    }
}

impl Default for FrameEditPane {
    fn default() -> Self {
        Self::new(Keymap::new())
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
                                self.settings.bpm = v.clamp(20, 300);
                            }
                        }
                        Field::Tuning => {
                            if let Ok(v) = text.parse::<f32>() {
                                self.settings.tuning_a4 = v.clamp(400.0, 480.0);
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
                        Field::Bpm => format!("{}", self.settings.bpm),
                        Field::Tuning => format!("{:.1}", self.settings.tuning_a4),
                        _ => unreachable!(),
                    };
                    self.edit_input.set_value(&val);
                    self.edit_input.set_focused(true);
                    self.editing = true;
                    Action::None
                } else {
                    Action::Session(SessionAction::UpdateSession(self.settings.clone()))
                }
            }
            Some("cancel") => {
                Action::Nav(NavAction::SwitchPane("instrument"))
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let rect = center_rect(area, 50, 13);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Session ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let label_col = inner.x + 2;
        let value_col = label_col + 15;

        for (i, field) in FIELDS.iter().enumerate() {
            let y = inner.y + 1 + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let is_selected = i == self.selected;
            let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));

            // Indicator
            if is_selected {
                let ind_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                if let Some(cell) = buf.cell_mut((label_col, y)) {
                    cell.set_char('>').set_style(ind_style);
                }
            }

            // Label
            let label_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::CYAN))
            };
            let label = format!("{:14}", Self::field_label(*field));
            Paragraph::new(Line::from(Span::styled(label, label_style)))
                .render(RatatuiRect::new(label_col + 2, y, 14, 1), buf);

            // Value
            if is_selected && self.editing {
                // Render TextInput inline
                self.edit_input.render_buf(buf, value_col, y, inner.width.saturating_sub(18));
            } else {
                let val_style = if is_selected {
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE))
                };
                let val = self.field_value(*field);
                Paragraph::new(Line::from(Span::styled(&val, val_style)))
                    .render(RatatuiRect::new(value_col, y, inner.width.saturating_sub(18), 1), buf);

                // Fill rest of line with selection bg
                if is_selected {
                    let fill_start = value_col + val.len() as u16;
                    let fill_end = inner.x + inner.width;
                    for x in fill_start..fill_end {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(' ').set_style(sel_bg);
                        }
                    }
                }
            }
        }

        // Help
        let help_y = rect.y + rect.height - 2;
        if help_y < area.y + area.height {
            let help = if self.editing {
                "Enter: confirm | Esc: cancel"
            } else {
                "Left/Right: adjust | Enter: type/confirm | Esc: cancel"
            };
            Paragraph::new(Line::from(Span::styled(
                help,
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ))).render(RatatuiRect::new(inner.x + 2, help_y, inner.width.saturating_sub(2), 1), buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn on_enter(&mut self, state: &AppState) {
        self.set_settings(state.session.musical_settings());
    }

    fn wants_exclusive_input(&self) -> bool {
        self.editing
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
