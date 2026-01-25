use crate::ui::{Color, Graphics, InputEvent, KeyCode, Style};

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
}

impl TextInput {
    pub fn new(label: &str) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            placeholder: String::new(),
            focused: false,
            label: label.to_string(),
        }
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self.cursor = self.value.len();
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

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
                self.value.insert(self.cursor, ch);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.value.len();
                true
            }
            _ => false,
        }
    }

    /// Render the text input at the given position
    /// Returns the height used (always 1 for single-line input)
    pub fn render(&self, g: &mut dyn Graphics, x: u16, y: u16, width: u16) -> u16 {
        // Draw label
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(x, y, &self.label);

        let input_x = x + self.label.len() as u16 + 1;
        let input_width = width.saturating_sub(self.label.len() as u16 + 1);

        // Draw input background/border
        let border_style = if self.focused {
            Style::new().fg(Color::BLUE)
        } else {
            Style::new().fg(Color::GRAY)
        };
        g.set_style(border_style);
        g.put_char(input_x, y, '[');
        g.put_char(input_x + input_width - 1, y, ']');

        // Draw content or placeholder
        let content_x = input_x + 1;
        let content_width = input_width.saturating_sub(2) as usize;

        if self.value.is_empty() && !self.focused {
            g.set_style(Style::new().fg(Color::GRAY));
            let placeholder: String = self.placeholder.chars().take(content_width).collect();
            g.put_str(content_x, y, &placeholder);
        } else {
            g.set_style(Style::new().fg(Color::BLACK));
            let display: String = self.value.chars().take(content_width).collect();
            g.put_str(content_x, y, &display);

            // Draw cursor if focused
            if self.focused {
                let cursor_x = content_x + self.cursor.min(content_width) as u16;
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                let cursor_char = self.value.chars().nth(self.cursor).unwrap_or(' ');
                g.put_char(cursor_x, y, cursor_char);
            }
        }

        1
    }
}
