use std::any::Any;
use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::widgets::TextInput;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, NavAction, Pane, SessionAction, Style};

pub struct SaveAsPane {
    keymap: Keymap,
    text_input: TextInput,
    error: Option<String>,
}

impl SaveAsPane {
    pub fn new(keymap: Keymap) -> Self {
        let mut text_input = TextInput::new("");
        text_input.set_focused(true);
        Self {
            keymap,
            text_input,
            error: None,
        }
    }

    /// Reset state when opening
    pub fn reset(&mut self, default_name: &str) {
        self.text_input.set_value(default_name);
        self.text_input.select_all();
        self.text_input.set_focused(true);
        self.error = None;
    }

    fn projects_dir() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("imbolc")
                .join("projects")
        } else {
            PathBuf::from("projects")
        }
    }
}

impl Pane for SaveAsPane {
    fn id(&self) -> &'static str {
        "save_as"
    }

    fn handle_action(&mut self, _action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        Action::None
    }

    fn handle_raw_input(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        match event.key {
            KeyCode::Enter => {
                let name = self.text_input.value().trim().to_string();
                if name.is_empty() {
                    self.error = Some("Name cannot be empty".to_string());
                    return Action::None;
                }

                let dir = Self::projects_dir();
                let path = dir.join(format!("{}.sqlite", name));
                Action::Session(SessionAction::SaveAs(path))
            }
            KeyCode::Escape => {
                Action::Nav(NavAction::PopPane)
            }
            _ => {
                // Delegate text editing to rat-widget TextInput
                self.text_input.handle_input(event);
                self.error = None;
                Action::None
            }
        }
    }

    fn render(&mut self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let width = 46_u16.min(area.width.saturating_sub(4));
        let height = if self.error.is_some() { 8 } else { 7 };
        let rect = center_rect(area, width, height);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Save As ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        // Label
        let label_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let label_area = RatatuiRect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1);
        Paragraph::new(Line::from(Span::styled("Project name:", label_style)))
            .render(label_area, buf);

        // Text input field (rat-widget backed)
        let field_y = inner.y + 2;
        let field_width = inner.width.saturating_sub(2);
        self.text_input.render_buf(buf, inner.x + 1, field_y, field_width);

        // Error message
        if let Some(ref error) = self.error {
            let err_y = inner.y + 3;
            if err_y < inner.y + inner.height {
                let err_area = RatatuiRect::new(inner.x + 1, err_y, inner.width.saturating_sub(2), 1);
                let err_style = ratatui::style::Style::from(Style::new().fg(Color::MUTE_COLOR));
                Paragraph::new(Line::from(Span::styled(error, err_style)))
                    .render(err_area, buf);
            }
        }

        // Footer
        let footer_y = rect.y + rect.height.saturating_sub(2);
        if footer_y < area.y + area.height {
            let footer_area = RatatuiRect::new(inner.x + 1, footer_y, inner.width.saturating_sub(2), 1);
            Paragraph::new(Line::from(Span::styled(
                "[Enter] Save  [Esc] Cancel",
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ))).render(footer_area, buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
