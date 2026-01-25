use crate::state::ModuleType;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

/// Represents an item in the module selection list
#[derive(Debug, Clone)]
struct ModuleItem {
    module_type: Option<ModuleType>,
    display_name: String,
    is_header: bool,
}

impl ModuleItem {
    fn header(name: &str) -> Self {
        Self {
            module_type: None,
            display_name: name.to_string(),
            is_header: true,
        }
    }

    fn module(module_type: ModuleType) -> Self {
        Self {
            module_type: Some(module_type.clone()),
            display_name: module_type.name().to_string(),
            is_header: false,
        }
    }
}

pub struct AddPane {
    keymap: Keymap,
    items: Vec<ModuleItem>,
    selected: usize,
}

impl AddPane {
    pub fn new() -> Self {
        let items = vec![
            ModuleItem::header("Oscillators:"),
            ModuleItem::module(ModuleType::SawOsc),
            ModuleItem::module(ModuleType::SinOsc),
            ModuleItem::module(ModuleType::SqrOsc),
            ModuleItem::module(ModuleType::TriOsc),
            ModuleItem::header(""),
            ModuleItem::header("Filters:"),
            ModuleItem::module(ModuleType::Lpf),
            ModuleItem::module(ModuleType::Hpf),
            ModuleItem::module(ModuleType::Bpf),
            ModuleItem::header(""),
            ModuleItem::header("Envelopes:"),
            ModuleItem::module(ModuleType::AdsrEnv),
            ModuleItem::header(""),
            ModuleItem::header("Modulation:"),
            ModuleItem::module(ModuleType::Lfo),
            ModuleItem::header(""),
            ModuleItem::header("Effects:"),
            ModuleItem::module(ModuleType::Delay),
            ModuleItem::module(ModuleType::Reverb),
            ModuleItem::header(""),
            ModuleItem::header("Output:"),
            ModuleItem::module(ModuleType::Output),
        ];

        // Find first selectable item (skip headers)
        let initial_selected = items
            .iter()
            .position(|item| !item.is_header)
            .unwrap_or(0);

        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Enter, "confirm", "Add selected module")
                .bind_key(KeyCode::Escape, "cancel", "Cancel and return to rack")
                .bind('n', "next", "Next module")
                .bind('p', "prev", "Previous module")
                .bind('j', "next", "Next module (vim)")
                .bind('k', "prev", "Previous module (vim)")
                .bind_key(KeyCode::Down, "next", "Next module")
                .bind_key(KeyCode::Up, "prev", "Previous module"),
            items,
            selected: initial_selected,
        }
    }

    fn select_next(&mut self) {
        let start = self.selected;
        loop {
            self.selected = (self.selected + 1) % self.items.len();
            if !self.items[self.selected].is_header || self.selected == start {
                break;
            }
        }
    }

    fn select_prev(&mut self) {
        let start = self.selected;
        loop {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
            if !self.items[self.selected].is_header || self.selected == start {
                break;
            }
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
                // Return to rack pane - rack pane will handle the actual module addition
                Action::SwitchPane("rack")
            }
            Some("cancel") => Action::SwitchPane("rack"),
            Some("next") => {
                self.select_next();
                Action::None
            }
            Some("prev") => {
                self.select_prev();
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

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" Add Module "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(content_x, content_y, "Select module type:");

        // Module list
        let list_y = content_y + 2;
        let max_visible = (rect.height - 6) as usize;

        for (i, item) in self.items.iter().enumerate() {
            let y = list_y + i as u16;
            if y >= rect.y + rect.height - 3 {
                break;
            }

            if item.is_header {
                // Render header
                g.set_style(Style::new().fg(Color::BLACK));
                g.put_str(content_x, y, &item.display_name);
            } else {
                // Render selectable module
                let is_selected = i == self.selected;

                // Selection indicator
                if is_selected {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                    g.put_str(content_x, y, ">");
                } else {
                    g.set_style(Style::new().fg(Color::BLACK));
                    g.put_str(content_x, y, " ");
                }

                // Module type short name (e.g., "SawOsc")
                if let Some(ref module_type) = item.module_type {
                    let type_str = format!("{:?}", module_type);
                    g.put_str(content_x + 2, y, &format!("{:12}", type_str));

                    // Module description
                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                    } else {
                        g.set_style(Style::new().fg(Color::GRAY));
                    }
                    g.put_str(content_x + 15, y, &item.display_name);

                    // Clear to end of selection if selected
                    if is_selected {
                        let line_end = content_x + 15 + item.display_name.len() as u16;
                        for x in line_end..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                }
            }
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(content_x, help_y, "Enter: add | Escape: cancel | n/p: navigate");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}
