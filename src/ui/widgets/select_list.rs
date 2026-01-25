use crate::ui::{Color, Graphics, InputEvent, KeyCode, Style};

/// An item in a select list
#[derive(Clone)]
pub struct ListItem {
    pub id: String,
    pub label: String,
}

impl ListItem {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
        }
    }
}

/// A selectable list widget
pub struct SelectList {
    items: Vec<ListItem>,
    selected: usize,
    focused: bool,
    title: String,
}

impl SelectList {
    pub fn new(title: &str) -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            focused: false,
            title: title.to_string(),
        }
    }

    pub fn with_items(mut self, items: Vec<ListItem>) -> Self {
        self.items = items;
        self
    }

    pub fn add_item(&mut self, id: &str, label: &str) {
        self.items.push(ListItem::new(id, label));
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected_item(&self) -> Option<&ListItem> {
        self.items.get(self.selected)
    }

    pub fn select_by_id(&mut self, id: &str) -> bool {
        if let Some(idx) = self.items.iter().position(|item| item.id == id) {
            self.selected = idx;
            true
        } else {
            false
        }
    }

    /// Handle input, returns true if the event was consumed
    pub fn handle_input(&mut self, event: &InputEvent) -> bool {
        if !self.focused || self.items.is_empty() {
            return false;
        }

        match event.key {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            KeyCode::Down => {
                if self.selected < self.items.len() - 1 {
                    self.selected += 1;
                }
                true
            }
            KeyCode::Home => {
                self.selected = 0;
                true
            }
            KeyCode::End => {
                self.selected = self.items.len().saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    /// Render the select list at the given position
    /// Returns the height used
    pub fn render(&self, g: &mut dyn Graphics, x: u16, y: u16, width: u16, max_height: u16) -> u16 {
        let mut current_y = y;

        // Draw title
        let title_style = if self.focused {
            Style::new().fg(Color::BLUE)
        } else {
            Style::new().fg(Color::BLACK)
        };
        g.set_style(title_style);
        g.put_str(x, current_y, &self.title);
        current_y += 1;

        // Draw items
        let visible_items = (max_height - 1) as usize;
        let start_idx = if self.selected >= visible_items {
            self.selected - visible_items + 1
        } else {
            0
        };

        for (i, item) in self.items.iter().enumerate().skip(start_idx) {
            if current_y >= y + max_height {
                break;
            }

            let is_selected = i == self.selected;

            // Selection indicator and styling
            if is_selected && self.focused {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLUE));
                g.put_str(x, current_y, "> ");
            } else if is_selected {
                g.set_style(Style::new().fg(Color::BLACK));
                g.put_str(x, current_y, "> ");
            } else {
                g.set_style(Style::new().fg(Color::BLACK));
                g.put_str(x, current_y, "  ");
            }

            // Item label
            let label_width = (width - 2) as usize;
            let label: String = item.label.chars().take(label_width).collect();
            g.put_str(x + 2, current_y, &label);

            // Reset style after selected item
            if is_selected && self.focused {
                // Fill rest of line with selection background
                let remaining = label_width.saturating_sub(label.len());
                for i in 0..remaining {
                    g.put_char(x + 2 + label.len() as u16 + i as u16, current_y, ' ');
                }
            }

            current_y += 1;
        }

        current_y - y
    }
}
