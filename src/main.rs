mod ui;

use std::time::Duration;

use ui::{
    widgets::{ListItem, SelectList, TextInput},
    Action, Color, Graphics, InputEvent, InputSource, KeyCode, Keymap, Pane, PaneManager,
    RatatuiBackend, Rect, Style,
};

/// Demo pane showing widgets
struct DemoPane {
    keymap: Keymap,
    name_input: TextInput,
    email_input: TextInput,
    module_list: SelectList,
    focus_index: Option<usize>, // None = nothing, Some(0) = name, Some(1) = email, Some(2) = list
}

impl DemoPane {
    fn new() -> Self {
        let name_input = TextInput::new("Name:")
            .with_placeholder("Enter your name");

        let email_input = TextInput::new("Email:")
            .with_placeholder("user@example.com");

        let module_list = SelectList::new("Modules:")
            .with_items(vec![
                ListItem::new("osc", "Oscillator"),
                ListItem::new("filter", "Filter"),
                ListItem::new("env", "Envelope"),
                ListItem::new("lfo", "LFO"),
                ListItem::new("delay", "Delay"),
                ListItem::new("reverb", "Reverb"),
                ListItem::new("chorus", "Chorus"),
                ListItem::new("distortion", "Distortion"),
            ]);

        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit the application")
                .bind_key(KeyCode::Tab, "next_field", "Move to next field")
                .bind_key(KeyCode::Enter, "select", "Select current item")
                .bind_key(KeyCode::Escape, "cancel", "Cancel/Go back"),
            name_input,
            email_input,
            module_list,
            focus_index: None, // Nothing focused initially
        }
    }

    fn update_focus(&mut self) {
        self.name_input.set_focused(self.focus_index == Some(0));
        self.email_input.set_focused(self.focus_index == Some(1));
        self.module_list.set_focused(self.focus_index == Some(2));
    }

    fn next_focus(&mut self) {
        self.focus_index = match self.focus_index {
            None => Some(0),     // Nothing -> first field
            Some(2) => None,     // Last -> nothing (cycle complete)
            Some(n) => Some(n + 1),
        };
        self.update_focus();
    }
}

impl Pane for DemoPane {
    fn id(&self) -> &'static str {
        "demo"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        // Let focused widget handle input first
        let consumed = match self.focus_index {
            Some(0) => self.name_input.handle_input(&event),
            Some(1) => self.email_input.handle_input(&event),
            Some(2) => self.module_list.handle_input(&event),
            _ => false,
        };

        if consumed {
            return Action::None;
        }

        // Then check global keybindings
        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("next_field") => {
                self.next_focus();
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

        // Draw main box
        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" tuidaw - Demo "));

        // Content area (inside the box)
        let content_x = rect.x + 2;
        let content_y = rect.y + 2;
        let content_width = rect.width - 4;

        // Draw text inputs
        let mut y = content_y;

        self.name_input.render(g, content_x, y, content_width / 2);
        y += 2;

        self.email_input.render(g, content_x, y, content_width / 2);
        y += 3;

        // Draw select list
        self.module_list.render(g, content_x, y, content_width / 2, 12);

        // Draw info panel on the right
        let info_x = content_x + content_width / 2 + 4;
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(info_x, content_y, "Current Values:");
        g.put_str(info_x, content_y + 2, &format!("Name: {}", self.name_input.value()));
        g.put_str(info_x, content_y + 3, &format!("Email: {}", self.email_input.value()));

        if let Some(item) = self.module_list.selected_item() {
            g.put_str(info_x, content_y + 4, &format!("Module: {}", item.label));
        }

        // Draw status/hint at bottom
        let help_y = rect.y + rect.height - 2;
        if self.focus_index.is_none() {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
            g.put_str(content_x, help_y, " Press Tab to start ");
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(content_x + 21, help_y, " | q: quit");
        } else {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(content_x, help_y, "Tab: next field | Enter: select | q: quit");
        }
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
    let mut panes = PaneManager::new(Box::new(DemoPane::new()));

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
