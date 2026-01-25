mod ui;

use std::time::Duration;

use ui::{
    Action, Color, Graphics, InputEvent, InputSource, KeyCode, Keymap, Pane, PaneManager,
    RatatuiBackend, Rect, Style,
};

/// Main pane showing the tuidaw title box
struct MainPane {
    keymap: Keymap,
}

impl MainPane {
    fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit the application")
                .bind('?', "help", "Show help"),
        }
    }
}

impl Pane for MainPane {
    fn id(&self) -> &'static str {
        "main"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("help") => {
                // TODO: switch to help pane
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 30;
        let box_height = 10;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" tuidaw "));

        // Show keybindings at bottom of box
        let help_y = rect.y + rect.height - 2;
        let help_text = "Press ? for help, q to quit";
        let help_x = rect.x + (rect.width.saturating_sub(help_text.len() as u16)) / 2;
        g.put_str(help_x, help_y, help_text);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}

fn main() -> std::io::Result<()> {
    let mut backend = RatatuiBackend::new()?;
    backend.start()?;

    let result = run(&mut backend);

    backend.stop()?;
    result
}

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let mut panes = PaneManager::new(Box::new(MainPane::new()));

    loop {
        // Poll for input
        if let Some(event) = backend.poll_event(Duration::from_millis(16)) {
            let action = panes.handle_input(event);
            if action == Action::Quit {
                break;
            }
        }

        // Render
        let mut frame = backend.begin_frame()?;
        panes.render(&mut frame);
        backend.end_frame(frame)?;
    }

    Ok(())
}
