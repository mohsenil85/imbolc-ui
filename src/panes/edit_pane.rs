use crate::state::{Param, ParamValue};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

pub struct EditPane {
    keymap: Keymap,
    module_name: String,
    module_type_name: String,
    params: Vec<Param>,
    selected_param: usize,
}

impl EditPane {
    pub fn new(module_name: String, module_type_name: &str, params: Vec<Param>) -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "done", "Done editing")
                .bind('n', "next", "Next parameter")
                .bind('p', "prev", "Previous parameter")
                .bind('j', "next", "Next parameter (vim)")
                .bind('k', "prev", "Previous parameter (vim)")
                .bind_key(KeyCode::Down, "next", "Next parameter")
                .bind_key(KeyCode::Up, "prev", "Previous parameter")
                .bind_key(KeyCode::Left, "decrease", "Decrease value")
                .bind_key(KeyCode::Right, "increase", "Increase value"),
            module_name,
            module_type_name: module_type_name.to_string(),
            params,
            selected_param: 0,
        }
    }

    fn adjust_param(&mut self, increase: bool) {
        if self.params.is_empty() {
            return;
        }

        let param = &mut self.params[self.selected_param];
        let range = param.max - param.min;

        match &mut param.value {
            ParamValue::Float(ref mut value) => {
                let delta = range * 0.05; // 5% of range
                if increase {
                    *value = (*value + delta).min(param.max);
                } else {
                    *value = (*value - delta).max(param.min);
                }
            }
            ParamValue::Int(ref mut value) => {
                if increase {
                    *value = (*value + 1).min(param.max as i32);
                } else {
                    *value = (*value - 1).max(param.min as i32);
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
}

impl Pane for EditPane {
    fn id(&self) -> &'static str {
        "edit"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("done") => Action::SwitchPane("rack"),
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
            Some("increase") => {
                self.adjust_param(true);
                Action::None
            }
            Some("decrease") => {
                self.adjust_param(false);
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

        // Draw box with title
        let title = format!(" Edit: {} ({}) ", self.module_name, self.module_type_name);
        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(&title));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(content_x, content_y, "Parameters:");

        // Draw parameters
        let list_y = content_y + 2;

        for (i, param) in self.params.iter().enumerate() {
            let y = list_y + i as u16;
            if y >= rect.y + rect.height - 3 {
                break;
            }

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

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(content_x, help_y, "Left/Right: adjust | n/p: select param | Escape: done");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}
