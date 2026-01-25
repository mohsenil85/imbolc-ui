mod core;
mod panes;
mod state;
mod ui;

use std::time::Duration;

use panes::{AddPane, RackPane};
use ui::{
    widgets::{ListItem, SelectList, TextInput},
    Action, Color, Graphics, InputEvent, InputSource, KeyCode, Keymap, Pane, PaneManager,
    RatatuiBackend, Rect, Style,
};

// ============================================================================
// Demo Pane - Form with widgets
// ============================================================================

struct DemoPane {
    keymap: Keymap,
    name_input: TextInput,
    email_input: TextInput,
    module_list: SelectList,
    focus_index: Option<usize>,
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
                .bind('2', "goto_keymap", "Go to Keymap demo")
                .bind_key(KeyCode::Tab, "next_field", "Move to next field")
                .bind_key(KeyCode::Enter, "select", "Select current item")
                .bind_key(KeyCode::Escape, "cancel", "Cancel/Go back"),
            name_input,
            email_input,
            module_list,
            focus_index: None,
        }
    }

    fn update_focus(&mut self) {
        self.name_input.set_focused(self.focus_index == Some(0));
        self.email_input.set_focused(self.focus_index == Some(1));
        self.module_list.set_focused(self.focus_index == Some(2));
    }

    fn next_focus(&mut self) {
        self.focus_index = match self.focus_index {
            None => Some(0),
            Some(2) => None,
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
            Some("goto_keymap") => Action::SwitchPane("keymap"),
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

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" [1] Form Demo "));

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
            g.put_str(content_x + 21, help_y, " | 2: Keymap demo | q: quit");
        } else {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(content_x, help_y, "Tab: next | 2: Keymap demo | q: quit");
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}

// ============================================================================
// Keymap Demo Pane - Shows introspectable keymaps
// ============================================================================

struct KeymapPane {
    keymap: Keymap,
    selected: usize,
}

impl KeymapPane {
    fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit the application")
                .bind('1', "goto_form", "Go to Form demo")
                .bind_key(KeyCode::Up, "move_up", "Move selection up")
                .bind_key(KeyCode::Down, "move_down", "Move selection down")
                .bind('p', "move_up", "Previous item (emacs)")
                .bind('n', "move_down", "Next item (emacs)")
                .bind('k', "move_up", "Move up (vim)")
                .bind('j', "move_down", "Move down (vim)")
                .bind('g', "goto_top", "Go to top")
                .bind('G', "goto_bottom", "Go to bottom")
                .bind('/', "search", "Search keybindings"),
            selected: 0,
        }
    }
}

impl Pane for KeymapPane {
    fn id(&self) -> &'static str {
        "keymap"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        let binding_count = self.keymap.bindings().len();

        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("goto_form") => Action::SwitchPane("demo"),
            Some("move_up") => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            Some("move_down") => {
                if self.selected < binding_count.saturating_sub(1) {
                    self.selected += 1;
                }
                Action::None
            }
            Some("goto_top") => {
                self.selected = 0;
                Action::None
            }
            Some("goto_bottom") => {
                self.selected = binding_count.saturating_sub(1);
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

        g.set_style(Style::new().fg(Color::BLACK));
        g.draw_box(rect, Some(" [2] Keymap Demo "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(content_x, content_y, "This pane's keybindings:");
        g.put_str(content_x, content_y + 1, "(navigate with arrows or j/k)");

        // Draw keymap entries
        let bindings = self.keymap.bindings();
        let list_y = content_y + 3;

        for (i, binding) in bindings.iter().enumerate() {
            let y = list_y + i as u16;
            if y >= rect.y + rect.height - 3 {
                break;
            }

            let is_selected = i == self.selected;

            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                g.put_str(content_x, y, "> ");
            } else {
                g.set_style(Style::new().fg(Color::BLACK));
                g.put_str(content_x, y, "  ");
            }

            // Key display
            let key_display = binding.pattern.display();
            g.put_str(content_x + 2, y, &format!("{:12}", key_display));

            // Action name
            g.put_str(content_x + 15, y, &format!("{:15}", binding.action));

            // Description
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
            } else {
                g.set_style(Style::new().fg(Color::GRAY));
            }
            g.put_str(content_x + 31, y, binding.description);

            // Clear to end of selection
            if is_selected {
                let desc_len = binding.description.len();
                for x in (content_x + 31 + desc_len as u16)..(rect.x + rect.width - 2) {
                    g.put_char(x, y, ' ');
                }
            }
        }

        // Draw help at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_str(content_x, help_y, "n/p or j/k: navigate | 1: Form demo | q: quit");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() -> std::io::Result<()> {
    let mut backend = RatatuiBackend::new()?;
    backend.start()?;

    let result = run(&mut backend);

    backend.stop()?;
    result
}

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let mut panes = PaneManager::new(Box::new(RackPane::new()));
    panes.add_pane(Box::new(AddPane::new()));
    panes.add_pane(Box::new(KeymapPane::new()));

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
