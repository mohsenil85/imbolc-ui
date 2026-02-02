use std::any::Any;
use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, NavAction, Pane, SessionAction, Style};

pub struct SaveAsPane {
    keymap: Keymap,
    name_buf: String,
    cursor: usize,
    error: Option<String>,
}

impl SaveAsPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            name_buf: String::new(),
            cursor: 0,
            error: None,
        }
    }

    /// Reset state when opening
    pub fn reset(&mut self, default_name: &str) {
        self.name_buf = default_name.to_string();
        self.cursor = self.name_buf.len();
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
            KeyCode::Char(c) => {
                // Only allow safe filename characters
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    self.name_buf.insert(self.cursor, c);
                    self.cursor += 1;
                    self.error = None;
                }
                Action::None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.name_buf.remove(self.cursor);
                    self.error = None;
                }
                Action::None
            }
            KeyCode::Delete => {
                if self.cursor < self.name_buf.len() {
                    self.name_buf.remove(self.cursor);
                    self.error = None;
                }
                Action::None
            }
            KeyCode::Left => {
                if self.cursor > 0 { self.cursor -= 1; }
                Action::None
            }
            KeyCode::Right => {
                if self.cursor < self.name_buf.len() { self.cursor += 1; }
                Action::None
            }
            KeyCode::Home => {
                self.cursor = 0;
                Action::None
            }
            KeyCode::End => {
                self.cursor = self.name_buf.len();
                Action::None
            }
            KeyCode::Enter => {
                let name = self.name_buf.trim().to_string();
                if name.is_empty() {
                    self.error = Some("Name cannot be empty".to_string());
                    return Action::None;
                }

                let dir = Self::projects_dir();
                let path = dir.join(format!("{}.sqlite", name));

                // Save even if file exists (overwrite) — could add confirm later
                Action::Session(SessionAction::SaveAs(path))
            }
            KeyCode::Escape => {
                Action::Nav(NavAction::PopPane)
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
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

        // Text input field
        let field_y = inner.y + 2;
        let field_width = inner.width.saturating_sub(4) as usize;
        let field_area = RatatuiRect::new(inner.x + 2, field_y, field_width as u16, 1);

        // Draw field background
        let field_bg = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::new(30, 30, 40)));
        for x in field_area.x..field_area.x + field_area.width {
            if let Some(cell) = buf.cell_mut((x, field_y)) {
                cell.set_char(' ').set_style(field_bg);
            }
        }

        // Draw text
        let display_text: String = self.name_buf.chars().take(field_width).collect();
        for (i, ch) in display_text.chars().enumerate() {
            let x = field_area.x + i as u16;
            if x < field_area.x + field_area.width {
                if let Some(cell) = buf.cell_mut((x, field_y)) {
                    cell.set_char(ch).set_style(field_bg);
                }
            }
        }

        // Draw cursor
        let cursor_x = field_area.x + self.cursor.min(field_width) as u16;
        if cursor_x < field_area.x + field_area.width {
            let cursor_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::WHITE));
            if let Some(cell) = buf.cell_mut((cursor_x, field_y)) {
                if self.cursor < self.name_buf.len() {
                    // Cursor on existing character
                    cell.set_style(cursor_style);
                } else {
                    cell.set_char(' ').set_style(cursor_style);
                }
            }
        }

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
