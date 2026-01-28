use std::any::Any;

use crate::state::OscType;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

pub struct AddPane {
    keymap: Keymap,
    items: Vec<OscType>,
    selected: usize,
}

impl AddPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Enter, "confirm", "Add selected strip")
                .bind_key(KeyCode::Escape, "cancel", "Cancel and return")
                .bind_key(KeyCode::Down, "next", "Next")
                .bind_key(KeyCode::Up, "prev", "Previous"),
            items: OscType::all(),
            selected: 0,
        }
    }
}

impl Default for AddPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for AddPane {
    fn id(&self) -> &'static str {
        "add"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("confirm") => {
                Action::AddStrip(self.items[self.selected])
            }
            Some("cancel") => Action::SwitchPane("strip"),
            Some("next") => {
                self.selected = (self.selected + 1) % self.items.len();
                Action::None
            }
            Some("prev") => {
                self.selected = if self.selected == 0 {
                    self.items.len() - 1
                } else {
                    self.selected - 1
                };
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::LIME));
        g.draw_box(rect, Some(" Add Strip "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        g.set_style(Style::new().fg(Color::LIME).bold());
        g.put_str(content_x, content_y, "Select oscillator type:");

        let list_y = content_y + 2;
        for (i, osc) in self.items.iter().enumerate() {
            let y = list_y + i as u16;
            let is_selected = i == self.selected;

            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, ">");
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x, y, " ");
            }

            if is_selected {
                g.set_style(Style::new().fg(Color::OSC_COLOR).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::OSC_COLOR));
            }
            g.put_str(content_x + 2, y, &format!("{:12}", osc.short_name()));

            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            g.put_str(content_x + 15, y, osc.name());

            if is_selected {
                g.set_style(Style::new().bg(Color::SELECTION_BG));
                let line_end = content_x + 15 + osc.name().len() as u16;
                for x in line_end..(rect.x + rect.width - 2) {
                    g.put_char(x, y, ' ');
                }
            }
        }

        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(content_x, help_y, "Enter: add | Escape: cancel | Up/Down: navigate");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
