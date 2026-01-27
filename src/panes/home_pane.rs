use std::any::Any;

use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

/// Menu item for the home screen
struct MenuItem {
    label: &'static str,
    description: &'static str,
    pane_id: &'static str,
}

pub struct HomePane {
    keymap: Keymap,
    selected: usize,
    items: Vec<MenuItem>,
}

impl HomePane {
    pub fn new() -> Self {
        let items = vec![
            MenuItem {
                label: "Rack",
                description: "Module rack - add and connect synthesizer modules",
                pane_id: "rack",
            },
            MenuItem {
                label: "Mixer",
                description: "Mixing console - adjust levels and routing",
                pane_id: "mixer",
            },
            MenuItem {
                label: "Server",
                description: "Audio server - start/stop and manage SuperCollider",
                pane_id: "server",
            },
        ];

        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Up, "up", "Previous item")
                .bind_key(KeyCode::Down, "down", "Next item")
                .bind('k', "up", "Previous item")
                .bind('j', "down", "Next item")
                .bind_key(KeyCode::Enter, "select", "Open selected pane")
                .bind(' ', "select", "Open selected pane")
                .bind('q', "quit", "Quit"),
            selected: 0,
            items,
        }
    }
}

impl Default for HomePane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for HomePane {
    fn id(&self) -> &'static str {
        "home"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("up") => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            Some("down") => {
                if self.selected < self.items.len() - 1 {
                    self.selected += 1;
                }
                Action::None
            }
            Some("select") => Action::SwitchPane(self.items[self.selected].pane_id),
            Some("quit") => Action::Quit,
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 50;
        let box_height = 12;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::MAGENTA));
        g.draw_box(rect, Some(" TUIDAW "));

        let content_x = rect.x + 3;
        let content_y = rect.y + 2;

        // Menu item colors
        let item_colors = [Color::CYAN, Color::PURPLE, Color::GOLD];

        // Render menu items
        for (i, item) in self.items.iter().enumerate() {
            let y = content_y + (i as u16 * 2);
            let is_selected = i == self.selected;
            let item_color = item_colors.get(i).copied().unwrap_or(Color::WHITE);

            // Selection indicator and label
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, &format!(" [{}] {} ", i + 1, item.label));
                // Description on next line
                g.set_style(Style::new().fg(Color::SKY_BLUE));
                g.put_str(content_x + 2, y + 1, item.description);
            } else {
                g.set_style(Style::new().fg(item_color));
                g.put_str(content_x, y, &format!(" [{}] {}", i + 1, item.label));
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x + 2, y + 1, item.description);
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(content_x, help_y, "[1-3] Jump  [Enter] Select  [q] Quit");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
