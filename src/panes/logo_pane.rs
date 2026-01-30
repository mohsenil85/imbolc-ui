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
            logo_content: include_str!("../../logo2.txt"),
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
        let width_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16; // Byte width for centering, might be approx

        let centered_rect = center_rect(inner, width, height);

        let color1 = Color::DEEP_GREEN;
        let color2 = Color::GOLD;
        let color3 = Color::RED;

        let text: Vec<Line> = lines.iter().enumerate()
            .map(|(y, l)| {
                let spans: Vec<Span> = l.char_indices().enumerate().map(|(x, (i, c))| {
                    let char_str = &l[i..i + c.len_utf8()];
                    
                    // Off-kilter diagonal factor: mostly vertical (y), but influenced by x
                    // Skew factor 0.5 means x contributes half as much as y
                    let y_f = if height > 1 { y as f32 / (height - 1) as f32 } else { 0.0 };
                    let x_f = if width_chars > 1 { x as f32 / (width_chars - 1) as f32 } else { 0.0 };
                    
                    let raw_factor = y_f + (x_f * 0.6); // 0.6 skew
                    let max_factor = 1.6;
                    let factor = (raw_factor / max_factor).clamp(0.0, 1.0);

                    // Shift midpoint to 0.8 to give even more space to the first transition (Purple->Cyan)
                    // and less to the second (Cyan->Pink)
                    let midpoint = 0.8;

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

                    Span::styled(char_str, ratatui::style::Style::from(Style::new().fg(color)))
                }).collect();
                
                Line::from(spans)
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
