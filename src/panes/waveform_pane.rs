use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, Pane, Style};

pub struct WaveformPane {
    keymap: Keymap,
}

impl WaveformPane {
    pub fn new(keymap: Keymap) -> Self {
        Self { keymap }
    }
}

impl Default for WaveformPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for WaveformPane {
    fn id(&self) -> &'static str {
        "waveform"
    }

    fn handle_input(&mut self, _event: InputEvent, _state: &AppState) -> Action {
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let rect = center_rect(area, 50, 10);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Waveform ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::AUDIO_IN_COLOR)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::AUDIO_IN_COLOR)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let text = "Waveform (coming soon)";
        let x = inner.x + (inner.width.saturating_sub(text.len() as u16)) / 2;
        let y = inner.y + inner.height / 2;
        Paragraph::new(Line::from(Span::styled(
            text,
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        )))
        .render(RatatuiRect::new(x, y, text.len() as u16, 1), buf);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
