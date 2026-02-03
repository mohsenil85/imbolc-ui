use std::any::Any;
use std::path::PathBuf;

use crate::audio::devices::{self, AudioDevice, AudioDeviceConfig};
use crate::audio::ServerStatus;
use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Rect, RenderBuf, Action, Color, InputEvent, KeyCode, Keymap, Pane, ServerAction, Style};

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
    log_lines: Vec<String>,
    log_path: PathBuf,
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

        let log_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("imbolc")
            .join("scsynth.log");

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
            log_lines: Vec::new(),
            log_path,
        }
    }

    pub fn set_status(&mut self, status: ServerStatus, message: &str) {
        self.status = status;
        self.message = message.to_string();
        self.refresh_log();
    }

    pub fn set_server_running(&mut self, running: bool) {
        self.server_running = running;
        self.refresh_log();
    }

    pub fn refresh_log(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.log_path) {
            self.log_lines = content
                .lines()
                .rev()
                .take(50)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(String::from)
                .collect();
        }
    }

    #[allow(dead_code)]
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
            "start" => Action::Server(ServerAction::Start {
                input_device: self.selected_input_device(),
                output_device: self.selected_output_device(),
            }),
            "stop" => Action::Server(ServerAction::Stop),
            "connect" => Action::Server(ServerAction::Connect),
            "disconnect" => Action::Server(ServerAction::Disconnect),
            "compile" => Action::Server(ServerAction::CompileSynthDefs),
            "load_synthdefs" => Action::Server(ServerAction::LoadSynthDefs),
            "record_master" => Action::Server(ServerAction::RecordMaster),
            "refresh_devices" => {
                self.refresh_devices();
                self.refresh_log();
                if self.server_running {
                    Action::Server(ServerAction::Restart {
                        input_device: self.selected_input_device(),
                        output_device: self.selected_output_device(),
                    })
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
                            return Action::Server(ServerAction::Restart {
                                input_device: self.selected_input_device(),
                                output_device: self.selected_output_device(),
                            });
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
                            return Action::Server(ServerAction::Restart {
                                input_device: self.selected_input_device(),
                                output_device: self.selected_output_device(),
                            });
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

    fn render(&mut self, area: Rect, buf: &mut RenderBuf, state: &AppState) {
        let output_devs = self.output_devices();
        let input_devs = self.input_devices();

        let rect = center_rect(area, 70, area.height.saturating_sub(2).max(15));

        let border_style = Style::new().fg(Color::GOLD);
        let inner = buf.draw_block(rect, " Audio Server (scsynth) ", border_style, border_style);

        let x = inner.x + 1;
        let w = inner.width.saturating_sub(2);
        let label_style = Style::new().fg(Color::CYAN);
        let mut y = inner.y + 1;

        // Server process status
        let (server_text, server_color) = if self.server_running {
            ("Running", Color::METER_LOW)
        } else {
            ("Stopped", Color::MUTE_COLOR)
        };
        buf.draw_line(
            Rect::new(x, y, w, 1),
            &[("Server:     ", label_style), (server_text, Style::new().fg(server_color).bold())],
        );
        y += 1;

        // Connection status
        let (status_text, status_color) = match self.status {
            ServerStatus::Stopped => ("Not connected", Color::DARK_GRAY),
            ServerStatus::Starting => ("Starting...", Color::ORANGE),
            ServerStatus::Running => ("Ready (not connected)", Color::SOLO_COLOR),
            ServerStatus::Connected => ("Connected", Color::METER_LOW),
            ServerStatus::Error => ("Error", Color::MUTE_COLOR),
        };
        buf.draw_line(
            Rect::new(x, y, w, 1),
            &[("Connection: ", label_style), (status_text, Style::new().fg(status_color).bold())],
        );
        y += 1;

        // Message
        if !self.message.is_empty() {
            let max_len = w as usize;
            let msg: String = self.message.chars().take(max_len).collect();
            buf.draw_line(
                Rect::new(x, y, w, 1),
                &[(&msg, Style::new().fg(Color::SKY_BLUE))],
            );
        }
        y += 1;

        // Recording status
        if state.recording {
            let mins = state.recording_secs / 60;
            let secs = state.recording_secs % 60;
            let rec_text = format!("REC {:02}:{:02}", mins, secs);
            buf.draw_line(
                Rect::new(x, y, w, 1),
                &[("Recording:  ", label_style), (&rec_text, Style::new().fg(Color::MUTE_COLOR).bold())],
            );
        }
        y += 1;

        // Output Device section
        let output_focused = self.focus == ServerPaneFocus::OutputDevice;
        let section_color = if output_focused { Color::GOLD } else { Color::DARK_GRAY };
        buf.draw_line(
            Rect::new(x, y, w, 1),
            &[("── Output Device ──", Style::new().fg(section_color))],
        );
        y += 1;

        // Render output device list
        y = self.render_device_list(buf, x, y, w, &output_devs, self.selected_output, output_focused);
        y += 1;

        // Input Device section
        let input_focused = self.focus == ServerPaneFocus::InputDevice;
        let section_color = if input_focused { Color::GOLD } else { Color::DARK_GRAY };
        buf.draw_line(
            Rect::new(x, y, w, 1),
            &[("── Input Device ──", Style::new().fg(section_color))],
        );
        y += 1;

        // Render input device list
        y = self.render_device_list(buf, x, y, w, &input_devs, self.selected_input, input_focused);
        y += 1;

        // Restart hint if config is dirty and server is running
        if self.device_config_dirty && self.server_running {
            if y < rect.y + rect.height - 3 {
                buf.draw_line(
                    Rect::new(x, y, w, 1),
                    &[("(restart server to apply device changes)", Style::new().fg(Color::ORANGE))],
                );
                y += 1;
            }
        }

        // Server log section
        let help_lines_count: u16 = 2;
        let bottom_reserved = help_lines_count + 2; // help + border + gap
        let log_bottom = rect.y + rect.height - bottom_reserved;
        if y < log_bottom {
            buf.draw_line(
                Rect::new(x, y, w, 1),
                &[("── Server Log ──", Style::new().fg(Color::DARK_GRAY))],
            );
            y += 1;

            let log_style = Style::new().fg(Color::DARK_GRAY);
            let available = (log_bottom.saturating_sub(y)) as usize;
            let skip = self.log_lines.len().saturating_sub(available);
            for line_text in self.log_lines.iter().skip(skip) {
                if y >= log_bottom {
                    break;
                }
                let truncated: String = line_text.chars().take(w as usize).collect();
                buf.draw_line(Rect::new(x, y, w, 1), &[(&truncated, log_style)]);
                y += 1;
            }
        }

        // Help text at bottom
        let _ = y;
        let help_style = Style::new().fg(Color::DARK_GRAY);
        let help_lines = [
            "s: start  k: kill  c: connect  d: disconnect  b: build  l: load",
            "r: refresh devices  Tab: next section",
        ];
        for (i, line_text) in help_lines.iter().enumerate() {
            let hy = rect.y + rect.height - (help_lines.len() as u16 + 1) + i as u16;
            if hy > inner.y && hy < rect.y + rect.height - 1 {
                buf.draw_line(Rect::new(x, hy, w, 1), &[(*line_text, help_style)]);
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
        buf: &mut RenderBuf,
        x: u16,
        mut y: u16,
        w: u16,
        devices: &[&AudioDevice],
        selected: usize,
        focused: bool,
    ) -> u16 {
        let normal_style = Style::new().fg(Color::WHITE);
        let selected_style = if focused {
            Style::new().fg(Color::GOLD).bold()
        } else {
            Style::new().fg(Color::WHITE).bold()
        };
        let marker_style = if focused {
            Style::new().fg(Color::GOLD)
        } else {
            Style::new().fg(Color::WHITE)
        };

        // "System Default" entry (index 0)
        let is_selected = selected == 0;
        let marker = if is_selected { "> " } else { "  " };
        let style = if is_selected { selected_style } else { normal_style };
        buf.draw_line(
            Rect::new(x, y, w, 1),
            &[(marker, marker_style), ("System Default", style)],
        );
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

            let info_style = Style::new().fg(Color::DARK_GRAY);

            buf.draw_line(
                Rect::new(x, y, w, 1),
                &[(marker, marker_style), (&device.name, style), (&suffix, info_style)],
            );
            y += 1;
        }

        y
    }
}
