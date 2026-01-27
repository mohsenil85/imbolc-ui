use std::any::Any;

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
            ModuleItem::header("Note Source:"),
            ModuleItem::module(ModuleType::Midi),
            ModuleItem::header(""),
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
                .bind_key(KeyCode::Down, "next", "Next module")
                .bind_key(KeyCode::Up, "prev", "Previous module")
                .bind_key(KeyCode::Home, "first", "First module")
                .bind_key(KeyCode::End, "last", "Last module"),
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
                // Return the selected module type - main loop will add it to rack
                if let Some(module_type) = self.items[self.selected].module_type {
                    Action::AddModule(module_type)
                } else {
                    Action::None
                }
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
            Some("first") => {
                self.selected = self.items.iter().position(|item| !item.is_header).unwrap_or(0);
                Action::None
            }
            Some("last") => {
                self.selected = self.items.iter().rposition(|item| !item.is_header).unwrap_or(0);
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
        g.draw_box(rect, Some(" Add Module "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::LIME).bold());
        g.put_str(content_x, content_y, "Select module type:");

        // Module list with viewport scrolling
        let list_y = content_y + 2;
        let max_visible = (rect.height - 7) as usize; // Leave room for title and help

        // Calculate scroll offset to keep selection visible
        let scroll_offset = if self.selected >= max_visible {
            self.selected - max_visible + 1
        } else {
            0
        };

        // Track current category color
        let mut current_category_color = Color::WHITE;

        for (i, item) in self.items.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;

            if item.is_header {
                // Determine category color from header name
                current_category_color = if item.display_name.contains("Note Source") {
                    Color::MIDI_COLOR
                } else if item.display_name.contains("Oscillator") {
                    Color::OSC_COLOR
                } else if item.display_name.contains("Filter") {
                    Color::FILTER_COLOR
                } else if item.display_name.contains("Envelope") {
                    Color::ENV_COLOR
                } else if item.display_name.contains("Modulation") {
                    Color::LFO_COLOR
                } else if item.display_name.contains("Effect") {
                    Color::FX_COLOR
                } else if item.display_name.contains("Output") {
                    Color::OUTPUT_COLOR
                } else {
                    Color::WHITE
                };

                // Render header with category color
                g.set_style(Style::new().fg(current_category_color).bold());
                g.put_str(content_x, y, &item.display_name);
            } else {
                // Render selectable module
                let is_selected = i == self.selected;

                // Selection indicator
                if is_selected {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                    g.put_str(content_x, y, ">");
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, y, " ");
                }

                // Module type short name (e.g., "SawOsc") with category color
                if let Some(ref module_type) = item.module_type {
                    let type_str = format!("{:?}", module_type);
                    if is_selected {
                        g.set_style(Style::new().fg(current_category_color).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(current_category_color));
                    }
                    g.put_str(content_x + 2, y, &format!("{:12}", type_str));

                    // Module description
                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(Color::DARK_GRAY));
                    }
                    g.put_str(content_x + 15, y, &item.display_name);

                    // Clear to end of selection if selected
                    if is_selected {
                        g.set_style(Style::new().bg(Color::SELECTION_BG));
                        let line_end = content_x + 15 + item.display_name.len() as u16;
                        for x in line_end..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                }
            }
        }

        // Scroll indicator if needed
        if scroll_offset > 0 {
            g.set_style(Style::new().fg(Color::ORANGE));
            g.put_str(rect.x + rect.width - 4, list_y, "...");
        }
        if scroll_offset + max_visible < self.items.len() {
            g.set_style(Style::new().fg(Color::ORANGE));
            g.put_str(rect.x + rect.width - 4, list_y + max_visible as u16 - 1, "...");
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(content_x, help_y, "Enter: add | Escape: cancel | Up/Down: navigate | Home/End: jump");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
