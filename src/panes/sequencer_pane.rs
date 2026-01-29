use std::any::Any;

use crate::state::AppState;
use crate::ui::{Action, Color, Graphics, InputEvent, Keymap, Pane, Rect, Style};

pub struct SequencerPane {
    keymap: Keymap,
}

impl SequencerPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit"),
        }
    }
}

impl Default for SequencerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for SequencerPane {
    fn id(&self) -> &'static str {
        "sequencer"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, _state: &AppState) {
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 50, 10);

        g.set_style(Style::new().fg(Color::PURPLE));
        g.draw_box(rect, Some(" Sequencer "));

        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(rect.x + 2, rect.y + 4, "Coming soon...");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
