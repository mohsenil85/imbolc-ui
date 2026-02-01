use ratatui::buffer::Buffer;

use crate::ui::{Color, InputEvent, KeyCode, Style};

/// A single-line text input widget
pub struct TextInput {
    /// The current text content
    value: String,
    /// Cursor position (character index)
    cursor: usize,
    /// Placeholder text shown when empty
    placeholder: String,
    /// Whether this input is focused
    focused: bool,
    /// Label shown before the input
    label: String,
    /// Whether all text is selected (next char typed replaces all)
    selected_all: bool,
}

impl TextInput {
    pub fn new(label: &str) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            placeholder: String::new(),
            focused: false,
            label: label.to_string(),
            selected_all: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    #[allow(dead_code)]
    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self.cursor = self.value.len();
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_string();
        self.cursor = self.value.len();
        self.selected_all = false;
    }

    /// Select all text — next typed character replaces everything
    pub fn select_all(&mut self) {
        self.selected_all = true;
        self.cursor = self.value.len();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[allow(dead_code)]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Handle input, returns true if the event was consumed
    pub fn handle_input(&mut self, event: &InputEvent) -> bool {
        if !self.focused {
            return false;
        }

        match event.key {
            KeyCode::Char(ch) if !event.modifiers.ctrl && !event.modifiers.alt => {
                if self.selected_all {
                    self.value.clear();
                    self.cursor = 0;
                    self.selected_all = false;
                }
                self.value.insert(self.cursor, ch);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.selected_all {
                    self.value.clear();
                    self.cursor = 0;
                    self.selected_all = false;
                } else if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Delete => {
                if self.selected_all {
                    self.value.clear();
                    self.cursor = 0;
                    self.selected_all = false;
                } else if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.selected_all = false;
                match event.key {
                    KeyCode::Left => { if self.cursor > 0 { self.cursor -= 1; } }
                    KeyCode::Right => { if self.cursor < self.value.len() { self.cursor += 1; } }
                    KeyCode::Home => { self.cursor = 0; }
                    KeyCode::End => { self.cursor = self.value.len(); }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    /// Render the text input into a ratatui buffer at the given position
    pub fn render_buf(&self, buf: &mut Buffer, x: u16, y: u16, width: u16) -> u16 {
        // Draw label
        let label_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
        for (j, ch) in self.label.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, y)) {
                cell.set_char(ch).set_style(label_style);
            }
        }

        let input_x = x + self.label.len() as u16 + 1;
        let input_width = width.saturating_sub(self.label.len() as u16 + 1);

        // Draw input brackets
        let border_style = if self.focused {
            ratatui::style::Style::from(Style::new().fg(Color::BLUE))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::GRAY))
        };
        if let Some(cell) = buf.cell_mut((input_x, y)) {
            cell.set_char('[').set_style(border_style);
        }
        if let Some(cell) = buf.cell_mut((input_x + input_width - 1, y)) {
            cell.set_char(']').set_style(border_style);
        }

        // Draw content or placeholder
        let content_x = input_x + 1;
        let content_width = input_width.saturating_sub(2) as usize;

        if self.value.is_empty() && !self.focused {
            let ph_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
            for (j, ch) in self.placeholder.chars().take(content_width).enumerate() {
                if let Some(cell) = buf.cell_mut((content_x + j as u16, y)) {
                    cell.set_char(ch).set_style(ph_style);
                }
            }
        } else if self.focused && self.selected_all && !self.value.is_empty() {
            // Draw all text with selection highlight
            let sel_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
            for (j, ch) in self.value.chars().take(content_width).enumerate() {
                if let Some(cell) = buf.cell_mut((content_x + j as u16, y)) {
                    cell.set_char(ch).set_style(sel_style);
                }
            }
        } else {
            let val_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
            for (j, ch) in self.value.chars().take(content_width).enumerate() {
                if let Some(cell) = buf.cell_mut((content_x + j as u16, y)) {
                    cell.set_char(ch).set_style(val_style);
                }
            }

            // Draw cursor if focused
            if self.focused {
                let cursor_x = content_x + self.cursor.min(content_width) as u16;
                let cursor_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                let cursor_char = self.value.chars().nth(self.cursor).unwrap_or(' ');
                if let Some(cell) = buf.cell_mut((cursor_x, y)) {
                    cell.set_char(cursor_char).set_style(cursor_style);
                }
            }
        }

        1
    }
}
