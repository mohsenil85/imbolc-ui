use std::any::Any;

use crate::state::{Connection, Module, ModuleId, ModuleType, Param, ParamValue, PortRef, RackState};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RackMode {
    Normal,
    ConnectSource,
    ConnectDest,
}

pub struct RackPane {
    keymap: Keymap,
    rack: RackState,
    mode: RackMode,
    selected_port: usize,
    pending_src: Option<PortRef>,
}

impl RackPane {
    pub fn new() -> Self {
        let mut rack = RackState::new();

        // Add some default modules to demonstrate functionality
        rack.add_module(ModuleType::SawOsc);
        rack.add_module(ModuleType::Lpf);
        rack.add_module(ModuleType::Output);

        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit the application")
                .bind_key(KeyCode::Down, "next", "Next module")
                .bind_key(KeyCode::Up, "prev", "Previous module")
                .bind_key(KeyCode::Home, "goto_top", "Go to top")
                .bind_key(KeyCode::End, "goto_bottom", "Go to bottom")
                .bind('a', "add", "Add module")
                .bind('d', "delete", "Delete module")
                .bind_key(KeyCode::Enter, "edit", "Edit module")
                .bind('c', "connect", "Connect modules")
                .bind('x', "disconnect", "Disconnect modules")
                .bind('w', "save", "Save rack")
                .bind('o', "load", "Load rack")
                .bind('s', "server", "Audio server")
                .bind('m', "mixer", "Mixer view")
                .bind_key(KeyCode::Left, "prev_port", "Previous port")
                .bind_key(KeyCode::Right, "next_port", "Next port")
                .bind_key(KeyCode::Tab, "next_port", "Next port")
                .bind_key(KeyCode::Escape, "cancel", "Cancel"),
            rack,
            mode: RackMode::Normal,
            selected_port: 0,
            pending_src: None,
        }
    }

    fn format_params(&self, module: &Module) -> String {
        let mut parts = Vec::new();

        // Show up to 2 key parameters
        for (i, param) in module.params.iter().take(2).enumerate() {
            if i >= 2 {
                break;
            }

            let value_str = match &param.value {
                ParamValue::Float(f) => format!("{:.1}", f),
                ParamValue::Int(i) => format!("{}", i),
                ParamValue::Bool(b) => format!("{}", b),
            };

            parts.push(format!("{}: {}", param.name, value_str));
        }

        parts.join("  ")
    }

    /// Get module data for editing (returns id, name, type_name, params)
    pub fn get_module_for_edit(&self, id: ModuleId) -> Option<(ModuleId, String, &'static str, Vec<Param>)> {
        self.rack.modules.get(&id).map(|m| {
            (m.id, m.name.clone(), m.module_type.name(), m.params.clone())
        })
    }

    /// Update a module's params
    pub fn update_module_params(&mut self, id: ModuleId, params: Vec<Param>) {
        if let Some(module) = self.rack.modules.get_mut(&id) {
            module.params = params;
        }
    }

    /// Get reference to rack state for saving
    pub fn rack(&self) -> &RackState {
        &self.rack
    }

    /// Get mutable reference to rack state
    pub fn rack_mut(&mut self) -> &mut RackState {
        &mut self.rack
    }

    /// Replace rack state (for loading)
    pub fn set_rack(&mut self, rack: RackState) {
        self.rack = rack;
        self.mode = RackMode::Normal;
        self.selected_port = 0;
        self.pending_src = None;
        // Select first module if any exist
        if !self.rack.order.is_empty() {
            self.rack.selected = Some(0);
        }
    }

    /// Get port count for the selected module
    fn selected_module_port_count(&self) -> usize {
        self.rack
            .selected_module()
            .map(|m| m.module_type.ports().len())
            .unwrap_or(0)
    }

    /// Get selected port for current module
    fn get_selected_port(&self) -> Option<PortRef> {
        let module = self.rack.selected_module()?;
        let ports = module.module_type.ports();
        ports.get(self.selected_port).map(|p| PortRef::new(module.id, p.name))
    }

    fn handle_normal_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("next") => {
                self.rack.select_next();
                Action::None
            }
            Some("prev") => {
                self.rack.select_prev();
                Action::None
            }
            Some("goto_top") => {
                if !self.rack.order.is_empty() {
                    self.rack.selected = Some(0);
                }
                Action::None
            }
            Some("goto_bottom") => {
                if !self.rack.order.is_empty() {
                    self.rack.selected = Some(self.rack.order.len() - 1);
                }
                Action::None
            }
            Some("add") => Action::SwitchPane("add"),
            Some("delete") => {
                if let Some(module) = self.rack.selected_module() {
                    let id = module.id;
                    self.rack.remove_module(id);
                }
                Action::None
            }
            Some("edit") => {
                if let Some(module) = self.rack.selected_module() {
                    Action::EditModule(module.id)
                } else {
                    Action::None
                }
            }
            Some("connect") => {
                if self.rack.selected_module().is_some() {
                    self.mode = RackMode::ConnectSource;
                    self.selected_port = 0;
                    self.pending_src = None;
                }
                Action::None
            }
            Some("disconnect") => {
                // Delete connections from/to selected module
                if let Some(module) = self.rack.selected_module() {
                    let module_id = module.id;
                    // Get first connection involving this module
                    let conn = self.rack.connections
                        .iter()
                        .find(|c| c.src.module_id == module_id || c.dst.module_id == module_id)
                        .cloned();
                    if let Some(connection) = conn {
                        return Action::RemoveConnection(connection);
                    }
                }
                Action::None
            }
            Some("save") => Action::SaveRack,
            Some("load") => Action::LoadRack,
            Some("server") => Action::SwitchPane("server"),
            Some("mixer") => Action::SwitchPane("mixer"),
            _ => Action::None,
        }
    }

    fn handle_connect_input(&mut self, event: InputEvent) -> Action {
        match self.keymap.lookup(&event) {
            Some("cancel") => {
                self.mode = RackMode::Normal;
                self.pending_src = None;
                self.selected_port = 0;
                Action::None
            }
            Some("next") => {
                self.rack.select_next();
                self.selected_port = 0;
                Action::None
            }
            Some("prev") => {
                self.rack.select_prev();
                self.selected_port = 0;
                Action::None
            }
            Some("next_port") => {
                let port_count = self.selected_module_port_count();
                if port_count > 0 {
                    self.selected_port = (self.selected_port + 1) % port_count;
                }
                Action::None
            }
            Some("prev_port") => {
                let port_count = self.selected_module_port_count();
                if port_count > 0 {
                    self.selected_port = if self.selected_port == 0 {
                        port_count - 1
                    } else {
                        self.selected_port - 1
                    };
                }
                Action::None
            }
            Some("edit") => {
                // Enter acts as confirm in connect mode
                if let Some(port_ref) = self.get_selected_port() {
                    match self.mode {
                        RackMode::ConnectSource => {
                            self.pending_src = Some(port_ref);
                            self.mode = RackMode::ConnectDest;
                            self.selected_port = 0;
                        }
                        RackMode::ConnectDest => {
                            if let Some(src) = self.pending_src.take() {
                                let connection = Connection::new(src, port_ref);
                                self.mode = RackMode::Normal;
                                self.selected_port = 0;
                                return Action::AddConnection(connection);
                            }
                        }
                        RackMode::Normal => {}
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

impl Default for RackPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for RackPane {
    fn id(&self) -> &'static str {
        "rack"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        match self.mode {
            RackMode::Normal => self.handle_normal_input(event),
            RackMode::ConnectSource | RackMode::ConnectDest => self.handle_connect_input(event),
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::BLACK));
        let title = match self.mode {
            RackMode::Normal => " Rack ",
            RackMode::ConnectSource => " Rack - Select Source ",
            RackMode::ConnectDest => " Rack - Select Destination ",
        };
        g.draw_box(rect, Some(title));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::BLACK));
        g.put_str(content_x, content_y, "Modules:");

        // Determine layout based on mode
        let in_connect_mode = self.mode != RackMode::Normal;
        let max_visible = if in_connect_mode {
            ((rect.height - 10) as usize).max(3) // Leave room for connection info
        } else {
            ((rect.height - 12) as usize).max(3) // Leave room for connections section
        };

        // Module list with viewport scrolling
        let list_y = content_y + 2;
        let selected_idx = self.rack.selected.unwrap_or(0);

        // Calculate scroll offset to keep selection visible
        let scroll_offset = if selected_idx >= max_visible {
            selected_idx - max_visible + 1
        } else {
            0
        };

        for (i, &module_id) in self.rack.order.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;

            if let Some(module) = self.rack.modules.get(&module_id) {
                let is_selected = self.rack.selected == Some(i);

                // Selection indicator
                if is_selected {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                    g.put_str(content_x, y, ">");
                } else {
                    g.set_style(Style::new().fg(Color::BLACK));
                    g.put_str(content_x, y, " ");
                }

                // Module name
                g.put_str(content_x + 2, y, &format!("{:16}", module.name));

                // Module type
                let type_name = format!("{:18}", module.module_type.name());
                g.put_str(content_x + 19, y, &type_name);

                if in_connect_mode {
                    // Show ports in connect mode
                    let ports = module.module_type.ports();
                    let mut port_x = content_x + 38;
                    for (port_idx, port) in ports.iter().enumerate() {
                        let is_port_selected = is_selected && port_idx == self.selected_port;
                        if is_port_selected {
                            g.set_style(Style::new().fg(Color::BLACK).bg(Color::WHITE));
                        } else if is_selected {
                            g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                        } else {
                            g.set_style(Style::new().fg(Color::GRAY));
                        }
                        let port_str = format!("[{}]", port.name);
                        g.put_str(port_x, y, &port_str);
                        port_x += port_str.len() as u16 + 1;
                    }
                    // Clear rest of line if selected
                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                        for x in port_x..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                } else {
                    // Show parameters in normal mode
                    let params_str = self.format_params(module);
                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::BLACK));
                    } else {
                        g.set_style(Style::new().fg(Color::GRAY));
                    }
                    g.put_str(content_x + 38, y, &params_str);

                    // Clear to end of selection if selected
                    if is_selected {
                        let line_end = content_x + 38 + params_str.len() as u16;
                        for x in line_end..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                }
            }
        }

        // Scroll indicators
        if scroll_offset > 0 {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(rect.x + rect.width - 4, list_y, "...");
        }
        if scroll_offset + max_visible < self.rack.order.len() {
            g.set_style(Style::new().fg(Color::GRAY));
            g.put_str(rect.x + rect.width - 4, list_y + max_visible as u16 - 1, "...");
        }

        // Show connections section in normal mode
        let conn_y = list_y + max_visible as u16 + 1;
        if !in_connect_mode && !self.rack.connections.is_empty() {
            g.set_style(Style::new().fg(Color::BLACK));
            g.put_str(content_x, conn_y, "Connections:");

            let mut y = conn_y + 1;
            for conn in self.rack.connections.iter().take(3) {
                g.set_style(Style::new().fg(Color::GRAY));
                // Format as module_name:port -> module_name:port
                let src_name = self.rack.modules.get(&conn.src.module_id)
                    .map(|m| m.name.as_str())
                    .unwrap_or("?");
                let dst_name = self.rack.modules.get(&conn.dst.module_id)
                    .map(|m| m.name.as_str())
                    .unwrap_or("?");
                let conn_str = format!("  {}:{} -> {}:{}", src_name, conn.src.port_name, dst_name, conn.dst.port_name);
                g.put_str(content_x, y, &conn_str);
                y += 1;
            }
            if self.rack.connections.len() > 3 {
                g.put_str(content_x, y, &format!("  ... and {} more", self.rack.connections.len() - 3));
            }
        }

        // Show connect mode status
        if in_connect_mode {
            g.set_style(Style::new().fg(Color::BLACK));
            let status_y = conn_y;
            if let Some(ref src) = self.pending_src {
                let src_name = self.rack.modules.get(&src.module_id)
                    .map(|m| m.name.as_str())
                    .unwrap_or("?");
                g.put_str(content_x, status_y, &format!("Source: {}:{}", src_name, src.port_name));
                g.put_str(content_x, status_y + 1, "Target: (select destination...)");
            } else {
                g.put_str(content_x, status_y, "Source: (select source port...)");
            }
        }

        // Help text at bottom
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::GRAY));
        let help_text = if in_connect_mode {
            "j/k: module | Tab/h/l: port | Enter: confirm | Esc: cancel"
        } else {
            "a: add | d: delete | e: edit | c: connect | x: disconnect | q: quit"
        };
        g.put_str(content_x, help_y, help_text);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn receive_action(&mut self, action: &Action) -> bool {
        match action {
            Action::AddModule(module_type) => {
                self.rack.add_module(*module_type);
                true
            }
            Action::UpdateModuleParams(id, params) => {
                self.update_module_params(*id, params.clone());
                true
            }
            Action::AddConnection(connection) => {
                let _ = self.rack.add_connection(connection.clone());
                true
            }
            Action::RemoveConnection(connection) => {
                self.rack.remove_connection(connection);
                true
            }
            _ => false,
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
