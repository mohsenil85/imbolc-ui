use std::any::Any;

use crate::state::AppState;
use crate::ui::{Rect, RenderBuf, Action, Color, InputEvent, Keymap, Pane, Style};
use crate::ui::layout_helpers::center_rect;

pub struct LogoPane {
    keymap: Keymap,
    logo_content: &'static str,
}

impl LogoPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            logo_content: include_str!("../../logo.txt"),
        }
    }
}

impl Pane for LogoPane {
    fn id(&self) -> &'static str {
        "logo"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "quit" => Action::Quit,
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut RenderBuf, _state: &AppState) {
        let border_style = Style::new().fg(Color::new(100, 80, 60));
        let inner = buf.draw_block(area, " Logo ", border_style, border_style);

        let lines: Vec<&str> = self.logo_content.lines().collect();
        let height = lines.len() as u16;
        let width_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

        let centered_rect = center_rect(inner, width, height);

        let color1 = Color::new(140, 50, 40);   // Red/Brown
        let color2 = Color::new(180, 130, 50);  // Gold/Brown
        let color3 = Color::new(50, 70, 30);    // Deep brownish green

        for (y, l) in lines.iter().enumerate() {
            for (x, c) in l.chars().enumerate() {
                let y_f = if height > 1 { y as f32 / (height - 1) as f32 } else { 0.0 };
                let x_f = if width_chars > 1 { x as f32 / (width_chars - 1) as f32 } else { 0.0 };

                let raw_factor = y_f + (x_f * 0.6);
                let max_factor = 1.6;
                let factor = (raw_factor / max_factor).clamp(0.0, 1.0);

                let midpoint = 0.5;

                let color = if factor < midpoint {
                    let f = factor / midpoint;
                    let r = (color1.r as f32 + (color2.r as f32 - color1.r as f32) * f) as u8;
                    let g = (color1.g as f32 + (color2.g as f32 - color1.g as f32) * f) as u8;
                    let b = (color1.b as f32 + (color2.b as f32 - color1.b as f32) * f) as u8;
                    Color::new(r, g, b)
                } else {
                    let f = (factor - midpoint) / (1.0 - midpoint);
                    let r = (color2.r as f32 + (color3.r as f32 - color2.r as f32) * f) as u8;
                    let g = (color2.g as f32 + (color3.g as f32 - color2.g as f32) * f) as u8;
                    let b = (color2.b as f32 + (color3.b as f32 - color2.b as f32) * f) as u8;
                    Color::new(r, g, b)
                };

                buf.set_cell(
                    centered_rect.x + x as u16,
                    centered_rect.y + y as u16,
                    c,
                    Style::new().fg(color),
                );
            }
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
