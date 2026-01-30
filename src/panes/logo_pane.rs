use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::{Action, Color, InputEvent, Keymap, Pane, Style};
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

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Logo ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        
        let inner = block.inner(area);
        block.render(area, buf);

        let lines: Vec<&str> = self.logo_content.lines().collect();
        let height = lines.len() as u16;
        let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

        let centered_rect = center_rect(inner, width, height);

        let color1 = Color::GREEN;
        let color2 = Color::PURPLE;
        let color3 = Color::WHITE;

        let text: Vec<Line> = lines.iter().enumerate()
            .map(|(i, l)| {
                let factor = if height > 1 {
                    i as f32 / (height - 1) as f32
                } else {
                    0.0
                };
                
                let color = if factor < 0.5 {
                    let f = factor * 2.0;
                    let r = (color1.r as f32 + (color2.r as f32 - color1.r as f32) * f) as u8;
                    let g = (color1.g as f32 + (color2.g as f32 - color1.g as f32) * f) as u8;
                    let b = (color1.b as f32 + (color2.b as f32 - color1.b as f32) * f) as u8;
                    Color::new(r, g, b)
                } else {
                    let f = (factor - 0.5) * 2.0;
                    let r = (color2.r as f32 + (color3.r as f32 - color2.r as f32) * f) as u8;
                    let g = (color2.g as f32 + (color3.g as f32 - color2.g as f32) * f) as u8;
                    let b = (color2.b as f32 + (color3.b as f32 - color2.b as f32) * f) as u8;
                    Color::new(r, g, b)
                };

                Line::from(Span::styled(*l, ratatui::style::Style::from(Style::new().fg(color))))
            })
            .collect();

        Paragraph::new(text).render(centered_rect, buf);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
