use std::any::Any;

use crate::state::{ModuleId, Param, ParamValue};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

pub struct EditPane {
    keymap: Keymap,
    module_id: Option<ModuleId>,
    module_name: String,
    module_type_name: String,
    params: Vec<Param>,
    selected_param: usize,
}

impl EditPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "done", "Done editing")
                .bind_key(KeyCode::Down, "next", "Next parameter")
                .bind_key(KeyCode::Up, "prev", "Previous parameter")
                .bind_key(KeyCode::Home, "first", "First parameter")
                .bind_key(KeyCode::End, "last", "Last parameter")
                .bind_key(KeyCode::Left, "decrease", "Decrease value")
                .bind_key(KeyCode::Right, "increase", "Increase value")
                .bind_key(KeyCode::PageUp, "increase_big", "Increase value +10%")
                .bind_key(KeyCode::PageDown, "decrease_big", "Decrease value -10%"),
            module_id: None,
            module_name: String::new(),
            module_type_name: String::new(),
            params: Vec::new(),
            selected_param: 0,
        }
    }

    /// Set the module to edit
    pub fn set_module(&mut self, id: ModuleId, name: String, type_name: &str, params: Vec<Param>) {
        self.module_id = Some(id);
        self.module_name = name;
        self.module_type_name = type_name.to_string();
        self.params = params;
        self.selected_param = 0;
    }

    /// Get the module ID being edited
    pub fn module_id(&self) -> Option<ModuleId> {
        self.module_id
    }

    /// Get the edited params (call when done editing)
    pub fn params(&self) -> &[Param] {
        &self.params
    }

    fn adjust_param(&mut self, increase: bool) {
        self.adjust_param_by(increase, 0.05);
    }

    fn adjust_param_big(&mut self, increase: bool) {
        self.adjust_param_by(increase, 0.10);
    }

    fn adjust_param_by(&mut self, increase: bool, fraction: f32) {
        if self.params.is_empty() {
            return;
        }

        let param = &mut self.params[self.selected_param];
        let range = param.max - param.min;

        match &mut param.value {
            ParamValue::Float(ref mut value) => {
                let delta = range * fraction;
                if increase {
                    *value = (*value + delta).min(param.max);
                } else {
                    *value = (*value - delta).max(param.min);
                }
            }
            ParamValue::Int(ref mut value) => {
                let delta = ((range * fraction) as i32).max(1);
                if increase {
                    *value = (*value + delta).min(param.max as i32);
                } else {
                    *value = (*value - delta).max(param.min as i32);
                }
            }
            ParamValue::Bool(ref mut value) => {
                *value = !*value;
            }
        }
    }

    fn render_slider(&self, param: &Param, width: usize) -> String {
        const SLIDER_WIDTH: usize = 30;

        match &param.value {
            ParamValue::Float(value) => {
                let normalized = (value - param.min) / (param.max - param.min);
                let pos = (normalized * SLIDER_WIDTH as f32) as usize;
                let pos = pos.min(SLIDER_WIDTH);

                let mut slider = String::with_capacity(SLIDER_WIDTH + 2);
                slider.push('[');

                for i in 0..SLIDER_WIDTH {
                    if i == pos {
                        slider.push('|');
                    } else if i < pos {
                        slider.push('=');
                    } else {
                        slider.push('-');
                    }
                }

                slider.push(']');
                slider
            }
            ParamValue::Int(value) => {
                let normalized = (*value as f32 - param.min) / (param.max - param.min);
                let pos = (normalized * SLIDER_WIDTH as f32) as usize;
                let pos = pos.min(SLIDER_WIDTH);

                let mut slider = String::with_capacity(SLIDER_WIDTH + 2);
                slider.push('[');

                for i in 0..SLIDER_WIDTH {
                    if i == pos {
                        slider.push('|');
                    } else if i < pos {
                        slider.push('=');
                    } else {
                        slider.push('-');
                    }
                }

                slider.push(']');
                slider
            }
            ParamValue::Bool(value) => {
                format!("[{}]", if *value { "ON " } else { "OFF" })
            }
        }
    }

    fn format_value(&self, param: &Param) -> String {
        match &param.value {
            ParamValue::Float(v) => format!("{:.1}", v),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => format!("{}", v),
        }
    }

    fn format_range(&self, param: &Param) -> String {
        match &param.value {
            ParamValue::Float(_) => format!("({:.0}-{:.0})", param.min, param.max),
            ParamValue::Int(_) => format!("({:.0}-{:.0})", param.min, param.max),
            ParamValue::Bool(_) => String::new(),
        }
    }

    fn emit_current_param(&self) -> Action {
        if let Some(id) = self.module_id {
            if let Some(param) = self.params.get(self.selected_param) {
                let value = match &param.value {
                    ParamValue::Float(v) => *v,
                    ParamValue::Int(v) => *v as f32,
                    ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                };
                return Action::SetModuleParam(id, param.name.clone(), value);
            }
        }
        Action::None
    }
}

impl Pane for EditPane {
    fn id(&self) -> &'static str {
        "edit"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("done") => {
                // Return updated params to be synced back to the rack
                if let Some(id) = self.module_id {
                    Action::UpdateModuleParams(id, self.params.clone())
                } else {
                    Action::SwitchPane("rack")
                }
            }
            Some("next") => {
                if !self.params.is_empty() {
                    self.selected_param = (self.selected_param + 1) % self.params.len();
                }
                Action::None
            }
            Some("prev") => {
                if !self.params.is_empty() {
                    if self.selected_param == 0 {
                        self.selected_param = self.params.len() - 1;
                    } else {
                        self.selected_param -= 1;
                    }
                }
                Action::None
            }
            Some("first") => {
                self.selected_param = 0;
                Action::None
            }
            Some("last") => {
                if !self.params.is_empty() {
                    self.selected_param = self.params.len() - 1;
                }
                Action::None
            }
            Some("increase") => {
                self.adjust_param(true);
                self.emit_current_param()
            }
            Some("decrease") => {
                self.adjust_param(false);
                self.emit_current_param()
            }
            Some("increase_big") => {
                self.adjust_param_big(true);
                self.emit_current_param()
            }
            Some("decrease_big") => {
                self.adjust_param_big(false);
                self.emit_current_param()
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        // Draw box with title
        let title = format!(" Edit: {} ({}) ", self.module_name, self.module_type_name);
        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(&title));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(content_x, content_y, "Parameters:");

        // Draw parameters with viewport scrolling
        let list_y = content_y + 2;
        let max_visible = (rect.height - 7) as usize;

        // Calculate scroll offset to keep selection visible
        let scroll_offset = if self.selected_param >= max_visible {
            self.selected_param - max_visible + 1
        } else {
            0
        };

        for (i, param) in self.params.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;

            let is_selected = i == self.selected_param;

            // Selection indicator
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                g.put_str(content_x, y, ">");
            } else {
                g.set_style(Style::new().fg(Color::BLACK));
                g.put_str(content_x, y, " ");
            }

            // Parameter name (left-aligned, 12 chars)
            let param_name = format!("{:12}", param.name);
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
            } else {
                g.set_style(Style::new().fg(Color::BLACK));
            }
            g.put_str(content_x + 2, y, &param_name);

            // Slider
            let slider = self.render_slider(param, 30);
            g.put_str(content_x + 15, y, &slider);

            // Value
            let value_str = self.format_value(param);
            let value_display = format!("{:10}", value_str);
            g.put_str(content_x + 48, y, &value_display);

            // Range
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
            } else {
                g.set_style(Style::new().fg(Color::GRAY));
            }
            let range_str = self.format_range(param);
            g.put_str(content_x + 59, y, &range_str);

            // Clear to end of selection if selected
            if is_selected {
                let line_end = content_x + 59 + range_str.len() as u16;
                for x in line_end..(rect.x + rect.width - 2) {
                    g.put_char(x, y, ' ');
                }
            }
        }

        // Scroll indicators
        if scroll_offset > 0 {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(rect.x + rect.width - 4, list_y, "...");
        }
        if scroll_offset + max_visible < self.params.len() {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(rect.x + rect.width - 4, list_y + max_visible as u16 - 1, "...");
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(content_x, help_y, "Left/Right: adjust | Up/Down: select | PgUp/PgDn: +/-10% | Esc: done");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Default for EditPane {
    fn default() -> Self {
        Self::new()
    }
}
