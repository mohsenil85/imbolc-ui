use std::any::Any;

use crate::state::AppState;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

pub struct HelpPane {
    keymap: Keymap,
    /// The keymap to display (from another pane)
    display_keymap: Vec<(String, String)>, // (key, description)
    /// Pane to return to when closing help
    return_to: &'static str,
    /// Title showing which pane's help this is
    title: String,
    /// Scroll offset for long keymaps
    scroll: usize,
}

impl HelpPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "close", "Close help")
                .bind('?', "close", "Close help")
                .bind_key(KeyCode::Up, "up", "Scroll up")
                .bind_key(KeyCode::Down, "down", "Scroll down")
                .bind('k', "up", "Scroll up")
                .bind('j', "down", "Scroll down")
                .bind_key(KeyCode::Home, "top", "Go to top")
                .bind_key(KeyCode::End, "bottom", "Go to bottom"),
            display_keymap: Vec::new(),
            return_to: "rack",
            title: String::new(),
            scroll: 0,
        }
    }

    /// Set the keymap to display and the pane to return to
    pub fn set_context(&mut self, pane_id: &'static str, pane_title: &str, keymap: &Keymap) {
        self.return_to = pane_id;
        self.title = pane_title.to_string();
        self.scroll = 0;

        // Convert keymap bindings to display format
        self.display_keymap = keymap
            .bindings()
            .iter()
            .map(|b| (b.pattern.display(), b.description.to_string()))
            .collect();
    }
}

impl Default for HelpPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for HelpPane {
    fn id(&self) -> &'static str {
        "help"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("close") => Action::SwitchPane(self.return_to),
            Some("up") => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
                Action::None
            }
            Some("down") => {
                self.scroll += 1;
                Action::None
            }
            Some("top") => {
                self.scroll = 0;
                Action::None
            }
            Some("bottom") => {
                self.scroll = self.display_keymap.len().saturating_sub(1);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, _state: &AppState) {
        let (width, height) = g.size();
        let box_width = 60;
        let box_height = 20;
        let rect = Rect::centered(width, height, box_width, box_height);

        let title = format!(" Help: {} ", self.title);
        g.set_style(Style::new().fg(Color::SKY_BLUE));
        g.draw_box(rect, Some(&title));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;
        let visible_lines = (rect.height - 5) as usize;

        // Clamp scroll to valid range
        let max_scroll = self.display_keymap.len().saturating_sub(visible_lines);
        let scroll = self.scroll.min(max_scroll);

        // Render keymap entries
        for (i, (key, desc)) in self.display_keymap.iter().skip(scroll).take(visible_lines).enumerate() {
            let y = content_y + i as u16;

            // Key
            g.set_style(Style::new().fg(Color::CYAN).bold());
            g.put_str(content_x, y, key);

            // Description
            g.set_style(Style::new().fg(Color::WHITE));
            let desc_x = content_x + 12;
            let max_desc_len = (rect.width - 16) as usize;
            let desc_truncated: String = desc.chars().take(max_desc_len).collect();
            g.put_str(desc_x, y, &desc_truncated);
        }

        // Scroll indicator
        if self.display_keymap.len() > visible_lines {
            let indicator_y = rect.y + rect.height - 3;
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            let indicator = format!(
                "{}-{}/{}",
                scroll + 1,
                (scroll + visible_lines).min(self.display_keymap.len()),
                self.display_keymap.len()
            );
            g.put_str(content_x, indicator_y, &indicator);
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(content_x, help_y, "[ESC/F1] Close  [Up/Down] Scroll");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
