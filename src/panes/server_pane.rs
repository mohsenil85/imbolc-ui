use std::any::Any;

use crate::audio::ServerStatus;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

pub struct ServerPane {
    keymap: Keymap,
    status: ServerStatus,
    message: String,
    server_running: bool,
}

impl ServerPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "back", "Return to rack")
                .bind('s', "start", "Start scsynth")
                .bind('k', "stop", "Kill scsynth")
                .bind('c', "connect", "Connect to server")
                .bind('d', "disconnect", "Disconnect")
                .bind('b', "compile", "Build synthdefs")
                .bind('l', "load", "Load synthdefs")
                .bind('h', "home", "Home screen"),
            status: ServerStatus::Stopped,
            message: String::new(),
            server_running: false,
        }
    }

    pub fn set_status(&mut self, status: ServerStatus, message: &str) {
        self.status = status;
        self.message = message.to_string();
    }

    pub fn set_server_running(&mut self, running: bool) {
        self.server_running = running;
    }
}

impl Default for ServerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for ServerPane {
    fn id(&self) -> &'static str {
        "server"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("back") => Action::SwitchPane("rack"),
            Some("start") => Action::StartServer,
            Some("stop") => Action::StopServer,
            Some("connect") => Action::ConnectServer,
            Some("disconnect") => Action::DisconnectServer,
            Some("compile") => Action::CompileSynthDefs,
            Some("load") => Action::LoadSynthDefs,
            Some("home") => Action::SwitchPane("home"),
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let rect = Rect::centered(width, height, 60, 15);

        g.set_style(Style::new().fg(Color::GOLD));
        g.draw_box(rect, Some(" Audio Server (scsynth) "));

        let x = rect.x + 2;
        let mut y = rect.y + 2;

        // Server process status
        g.set_style(Style::new().fg(Color::CYAN));
        g.put_str(x, y, "Server:     ");
        let (server_text, server_color) = if self.server_running {
            ("Running", Color::METER_LOW)
        } else {
            ("Stopped", Color::MUTE_COLOR)
        };
        g.set_style(Style::new().fg(server_color).bold());
        g.put_str(x + 12, y, server_text);
        y += 1;

        // Connection status
        g.set_style(Style::new().fg(Color::CYAN));
        g.put_str(x, y, "Connection: ");
        let (status_text, status_color) = match self.status {
            ServerStatus::Stopped => ("Not connected", Color::DARK_GRAY),
            ServerStatus::Starting => ("Starting...", Color::ORANGE),
            ServerStatus::Running => ("Ready (not connected)", Color::SOLO_COLOR),
            ServerStatus::Connected => ("Connected", Color::METER_LOW),
            ServerStatus::Error => ("Error", Color::MUTE_COLOR),
        };
        g.set_style(Style::new().fg(status_color).bold());
        g.put_str(x + 12, y, status_text);
        y += 2;

        // Message
        if !self.message.is_empty() {
            g.set_style(Style::new().fg(Color::SKY_BLUE));
            // Truncate message if too long
            let max_len = (rect.width - 4) as usize;
            let msg = if self.message.len() > max_len {
                &self.message[..max_len]
            } else {
                &self.message
            };
            g.put_str(x, y, msg);
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 5;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(x, help_y, "s: start server  k: kill server");
        g.put_str(x, help_y + 1, "c: connect       d: disconnect");
        g.put_str(x, help_y + 2, "b: build synths  l: load synths");
        g.put_str(x, help_y + 3, "Esc: back");
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
