use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::audio::devices::{self, AudioDevice, AudioDeviceConfig};
use crate::audio::ServerStatus;
use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, Pane, ServerAction, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ServerPaneFocus {
    Controls,
    OutputDevice,
    InputDevice,
}

pub struct ServerPane {
    keymap: Keymap,
    status: ServerStatus,
    message: String,
    server_running: bool,
    devices: Vec<AudioDevice>,
    selected_output: usize, // 0 = "System Default", 1+ = device index in output_devices()
    selected_input: usize,  // 0 = "System Default", 1+ = device index in input_devices()
    focus: ServerPaneFocus,
    /// Whether device selection changed since last server start
    device_config_dirty: bool,
}

impl ServerPane {
    pub fn new(keymap: Keymap) -> Self {
        let devices = devices::enumerate_devices();
        let config = devices::load_device_config();

        // Match saved config to device indices
        let selected_output = match &config.output_device {
            Some(name) => {
                let outputs: Vec<_> = devices.iter()
                    .filter(|d| d.output_channels.map_or(false, |c| c > 0))
                    .collect();
                outputs.iter().position(|d| d.name == *name)
                    .map(|i| i + 1)
                    .unwrap_or(0)
            }
            None => 0,
        };
        let selected_input = match &config.input_device {
            Some(name) => {
                let inputs: Vec<_> = devices.iter()
                    .filter(|d| d.input_channels.map_or(false, |c| c > 0))
                    .collect();
                inputs.iter().position(|d| d.name == *name)
                    .map(|i| i + 1)
                    .unwrap_or(0)
            }
            None => 0,
        };

        Self {
            keymap,
            status: ServerStatus::Stopped,
            message: String::new(),
            server_running: false,
            devices,
            selected_output,
            selected_input,
            focus: ServerPaneFocus::Controls,
            device_config_dirty: false,
        }
    }

    pub fn set_status(&mut self, status: ServerStatus, message: &str) {
        self.status = status;
        self.message = message.to_string();
    }

    pub fn set_server_running(&mut self, running: bool) {
        self.server_running = running;
    }

    pub fn clear_device_config_dirty(&mut self) {
        self.device_config_dirty = false;
    }

    /// Get the selected output device name (None = system default)
    pub fn selected_output_device(&self) -> Option<String> {
        if self.selected_output == 0 {
            return None;
        }
        self.output_devices().get(self.selected_output - 1).map(|d| d.name.clone())
    }

    /// Get the selected input device name (None = system default)
    pub fn selected_input_device(&self) -> Option<String> {
        if self.selected_input == 0 {
            return None;
        }
        self.input_devices().get(self.selected_input - 1).map(|d| d.name.clone())
    }

    fn output_devices(&self) -> Vec<&AudioDevice> {
        self.devices.iter()
            .filter(|d| d.output_channels.map_or(false, |c| c > 0))
            .collect()
    }

    fn input_devices(&self) -> Vec<&AudioDevice> {
        self.devices.iter()
            .filter(|d| d.input_channels.map_or(false, |c| c > 0))
            .collect()
    }

    fn refresh_devices(&mut self) {
        let old_output = self.selected_output_device();
        let old_input = self.selected_input_device();

        self.devices = devices::enumerate_devices();

        // Try to re-select previously selected devices
        self.selected_output = match &old_output {
            Some(name) => self.output_devices().iter()
                .position(|d| d.name == *name)
                .map(|i| i + 1)
                .unwrap_or(0),
            None => 0,
        };
        self.selected_input = match &old_input {
            Some(name) => self.input_devices().iter()
                .position(|d| d.name == *name)
                .map(|i| i + 1)
                .unwrap_or(0),
            None => 0,
        };
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            ServerPaneFocus::Controls => ServerPaneFocus::OutputDevice,
            ServerPaneFocus::OutputDevice => ServerPaneFocus::InputDevice,
            ServerPaneFocus::InputDevice => ServerPaneFocus::Controls,
        };
    }

    fn save_config(&self) {
        let config = AudioDeviceConfig {
            input_device: self.selected_input_device(),
            output_device: self.selected_output_device(),
        };
        devices::save_device_config(&config);
    }
}

impl Default for ServerPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for ServerPane {
    fn id(&self) -> &'static str {
        "server"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "start" => Action::Server(ServerAction::Start),
            "stop" => Action::Server(ServerAction::Stop),
            "connect" => Action::Server(ServerAction::Connect),
            "disconnect" => Action::Server(ServerAction::Disconnect),
            "compile" => Action::Server(ServerAction::CompileSynthDefs),
            "load_synthdefs" => Action::Server(ServerAction::LoadSynthDefs),
            "refresh_devices" => {
                self.refresh_devices();
                if self.server_running {
                    Action::Server(ServerAction::Restart)
                } else {
                    Action::None
                }
            }
            "next_section" => {
                self.cycle_focus();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_raw_input(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        // Focus-dependent navigation for Up/Down/Enter (not in layer)
        match self.focus {
            ServerPaneFocus::OutputDevice => {
                let count = self.output_devices().len() + 1; // +1 for "System Default"
                match event.key {
                    KeyCode::Up => {
                        self.selected_output = if self.selected_output == 0 {
                            count - 1
                        } else {
                            self.selected_output - 1
                        };
                        return Action::None;
                    }
                    KeyCode::Down => {
                        self.selected_output = (self.selected_output + 1) % count;
                        return Action::None;
                    }
                    KeyCode::Enter => {
                        self.save_config();
                        if self.server_running {
                            self.device_config_dirty = false;
                            return Action::Server(ServerAction::Restart);
                        } else {
                            self.device_config_dirty = true;
                            return Action::None;
                        }
                    }
                    _ => {}
                }
            }
            ServerPaneFocus::InputDevice => {
                let count = self.input_devices().len() + 1;
                match event.key {
                    KeyCode::Up => {
                        self.selected_input = if self.selected_input == 0 {
                            count - 1
                        } else {
                            self.selected_input - 1
                        };
                        return Action::None;
                    }
                    KeyCode::Down => {
                        self.selected_input = (self.selected_input + 1) % count;
                        return Action::None;
                    }
                    KeyCode::Enter => {
                        self.save_config();
                        if self.server_running {
                            self.device_config_dirty = false;
                            return Action::Server(ServerAction::Restart);
                        } else {
                            self.device_config_dirty = true;
                            return Action::None;
                        }
                    }
                    _ => {}
                }
            }
            ServerPaneFocus::Controls => {}
        }

        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let output_devs = self.output_devices();
        let input_devs = self.input_devices();

        // Calculate height: status(4) + output header(1) + output items + gap(1) + input header(1) + input items + gap(1) + help(2) + borders(2)
        let output_list_h = output_devs.len() + 1; // +1 for "System Default"
        let input_list_h = input_devs.len() + 1;
        let content_h = 4 + 1 + output_list_h + 1 + 1 + input_list_h + 1 + 2;
        let total_h = (content_h + 2).min(area.height as usize).max(15) as u16;

        let rect = center_rect(area, 70, total_h);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Audio Server (scsynth) ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::GOLD)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::GOLD)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let x = inner.x + 1;
        let w = inner.width.saturating_sub(2);
        let label_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN));
        let mut y = inner.y + 1;

        // Server process status
        let (server_text, server_color) = if self.server_running {
            ("Running", Color::METER_LOW)
        } else {
            ("Stopped", Color::MUTE_COLOR)
        };
        let server_line = Line::from(vec![
            Span::styled("Server:     ", label_style),
            Span::styled(server_text, ratatui::style::Style::from(Style::new().fg(server_color).bold())),
        ]);
        Paragraph::new(server_line).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        // Connection status
        let (status_text, status_color) = match self.status {
            ServerStatus::Stopped => ("Not connected", Color::DARK_GRAY),
            ServerStatus::Starting => ("Starting...", Color::ORANGE),
            ServerStatus::Running => ("Ready (not connected)", Color::SOLO_COLOR),
            ServerStatus::Connected => ("Connected", Color::METER_LOW),
            ServerStatus::Error => ("Error", Color::MUTE_COLOR),
        };
        let conn_line = Line::from(vec![
            Span::styled("Connection: ", label_style),
            Span::styled(status_text, ratatui::style::Style::from(Style::new().fg(status_color).bold())),
        ]);
        Paragraph::new(conn_line).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        // Message
        if !self.message.is_empty() {
            let max_len = w as usize;
            let msg: String = self.message.chars().take(max_len).collect();
            let msg_line = Line::from(Span::styled(
                msg,
                ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE)),
            ));
            Paragraph::new(msg_line).render(RatatuiRect::new(x, y, w, 1), buf);
        }
        y += 2;

        // Output Device section
        let output_focused = self.focus == ServerPaneFocus::OutputDevice;
        let section_color = if output_focused { Color::GOLD } else { Color::DARK_GRAY };
        let section_style = ratatui::style::Style::from(Style::new().fg(section_color));
        let header = Line::from(Span::styled("── Output Device ──", section_style));
        Paragraph::new(header).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        // Render output device list
        y = self.render_device_list(buf, x, y, w, &output_devs, self.selected_output, output_focused);
        y += 1;

        // Input Device section
        let input_focused = self.focus == ServerPaneFocus::InputDevice;
        let section_color = if input_focused { Color::GOLD } else { Color::DARK_GRAY };
        let section_style = ratatui::style::Style::from(Style::new().fg(section_color));
        let header = Line::from(Span::styled("── Input Device ──", section_style));
        Paragraph::new(header).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        // Render input device list
        y = self.render_device_list(buf, x, y, w, &input_devs, self.selected_input, input_focused);
        y += 1;

        // Restart hint if config is dirty and server is running
        if self.device_config_dirty && self.server_running {
            let hint_style = ratatui::style::Style::from(Style::new().fg(Color::ORANGE));
            let hint = Line::from(Span::styled("(restart server to apply device changes)", hint_style));
            if y < rect.y + rect.height - 3 {
                Paragraph::new(hint).render(RatatuiRect::new(x, y, w, 1), buf);
                y += 1;
            }
        }

        // Help text at bottom
        let _ = y;
        let help_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let help_lines = [
            "s: start  k: kill  c: connect  d: disconnect  b: build  l: load",
            "r: refresh devices  Tab: next section",
        ];
        for (i, line_text) in help_lines.iter().enumerate() {
            let hy = rect.y + rect.height - (help_lines.len() as u16 + 1) + i as u16;
            if hy > inner.y && hy < rect.y + rect.height - 1 {
                Paragraph::new(Line::from(Span::styled(*line_text, help_style)))
                    .render(RatatuiRect::new(x, hy, w, 1), buf);
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

impl ServerPane {
    /// Render a device list (shared between output and input sections).
    /// Returns the y position after the last rendered item.
    fn render_device_list(
        &self,
        buf: &mut Buffer,
        x: u16,
        mut y: u16,
        w: u16,
        devices: &[&AudioDevice],
        selected: usize,
        focused: bool,
    ) -> u16 {
        let normal_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
        let selected_style = if focused {
            ratatui::style::Style::from(Style::new().fg(Color::GOLD).bold())
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold())
        };
        let marker_style = if focused {
            ratatui::style::Style::from(Style::new().fg(Color::GOLD))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE))
        };

        // "System Default" entry (index 0)
        let is_selected = selected == 0;
        let marker = if is_selected { "> " } else { "  " };
        let style = if is_selected { selected_style } else { normal_style };
        let line = Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled("System Default", style),
        ]);
        Paragraph::new(line).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        // Device entries
        for (i, device) in devices.iter().enumerate() {
            let is_selected = selected == i + 1;
            let marker = if is_selected { "> " } else { "  " };
            let style = if is_selected { selected_style } else { normal_style };

            // Build device info suffix
            let mut info_parts = Vec::new();
            if let Some(sr) = device.sample_rate {
                info_parts.push(format!("{}Hz", sr));
            }
            if let Some(ch) = device.output_channels {
                if ch > 0 {
                    info_parts.push(format!("{}out", ch));
                }
            }
            if let Some(ch) = device.input_channels {
                if ch > 0 {
                    info_parts.push(format!("{}in", ch));
                }
            }

            let suffix = if info_parts.is_empty() {
                String::new()
            } else {
                format!("  ({})", info_parts.join(", "))
            };

            let info_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));

            let line = Line::from(vec![
                Span::styled(marker, marker_style),
                Span::styled(&device.name, style),
                Span::styled(suffix, info_style),
            ]);
            Paragraph::new(line).render(RatatuiRect::new(x, y, w, 1), buf);
            y += 1;
        }

        y
    }
}
