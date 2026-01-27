mod audio;
mod core;
mod panes;
mod state;
mod ui;

use std::any::Any;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use audio::AudioEngine;
use panes::{AddPane, EditPane, HelpPane, HomePane, MixerPane, PianoRollPane, RackPane, SequencerPane, ServerPane};
use state::{MixerSelection, RackState};
use ui::{
    widgets::{ListItem, SelectList, TextInput},
    Action, Color, Frame, Graphics, InputEvent, InputSource, KeyCode, Keymap, Pane, PaneManager,
    RatatuiBackend, Rect, Style,
};

/// Default path for rack save file
fn default_rack_path() -> PathBuf {
    // Use ~/.config/tuidaw/rack.tuidaw on Unix, current dir elsewhere
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("tuidaw")
            .join("rack.tuidaw")
    } else {
        PathBuf::from("rack.tuidaw")
    }
}

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

        g.set_style(Style::new().fg(Color::WHITE));
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
        g.set_style(Style::new().fg(Color::WHITE));
        g.put_str(info_x, content_y, "Current Values:");
        g.put_str(info_x, content_y + 2, &format!("Name: {}", self.name_input.value()));
        g.put_str(info_x, content_y + 3, &format!("Email: {}", self.email_input.value()));

        if let Some(item) = self.module_list.selected_item() {
            g.put_str(info_x, content_y + 4, &format!("Module: {}", item.label));
        }

        // Draw status/hint at bottom
        let help_y = rect.y + rect.height - 2;
        if self.focus_index.is_none() {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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

        g.set_style(Style::new().fg(Color::WHITE));
        g.draw_box(rect, Some(" [2] Keymap Demo "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Title
        g.set_style(Style::new().fg(Color::WHITE));
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
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                g.put_str(content_x, y, "> ");
            } else {
                g.set_style(Style::new().fg(Color::WHITE));
                g.put_str(content_x, y, "  ");
            }

            // Key display
            let key_display = binding.pattern.display();
            g.put_str(content_x + 2, y, &format!("{:12}", key_display));

            // Action name
            g.put_str(content_x + 15, y, &format!("{:15}", binding.action));

            // Description
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
    panes.add_pane(Box::new(HomePane::new()));
    panes.add_pane(Box::new(AddPane::new()));
    panes.add_pane(Box::new(EditPane::new()));
    panes.add_pane(Box::new(ServerPane::new()));
    panes.add_pane(Box::new(MixerPane::new()));
    panes.add_pane(Box::new(HelpPane::new()));
    panes.add_pane(Box::new(KeymapPane::new()));
    panes.add_pane(Box::new(PianoRollPane::new()));
    panes.add_pane(Box::new(SequencerPane::new()));

    let mut audio_engine = AudioEngine::new();
    let mut app_frame = Frame::new();
    let mut last_frame_time = Instant::now();
    // Active notes: (module_id, pitch, remaining_ticks)
    let mut active_notes: Vec<(u32, u8, u32)> = Vec::new();

    // Auto-start SuperCollider server
    {
        app_frame.push_message("SC: starting server...".to_string());
        match audio_engine.start_server() {
            Ok(()) => {
                app_frame.push_message("SC: server started on port 57110".to_string());
                if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                    server.set_status(audio::ServerStatus::Running, "Server started");
                    server.set_server_running(true);
                }
                // Auto-connect
                match audio_engine.connect("127.0.0.1:57110") {
                    Ok(()) => {
                        app_frame.push_message("SC: connected".to_string());
                        // Auto-load synthdefs
                        let synthdef_dir = std::path::Path::new("synthdefs");
                        if let Err(e) = audio_engine.load_synthdefs(synthdef_dir) {
                            app_frame.push_message(format!("SC: synthdef warning: {}", e));
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(
                                    audio::ServerStatus::Connected,
                                    &format!("Connected (synthdef warning: {})", e),
                                );
                            }
                        } else {
                            app_frame.push_message("SC: synthdefs loaded".to_string());
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(audio::ServerStatus::Connected, "Connected + synthdefs loaded");
                            }
                        }
                    }
                    Err(e) => {
                        app_frame.push_message(format!("SC: connect failed: {}", e));
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(audio::ServerStatus::Running, "Server running (connect failed)");
                        }
                    }
                }
            }
            Err(e) => {
                app_frame.push_message(format!("SC: start failed: {}", e));
            }
        }
    }

    loop {
        // Poll for input
        if let Some(event) = backend.poll_event(Duration::from_millis(16)) {
            // Global Ctrl-Q to quit
            if event.key == KeyCode::Char('q') && event.modifiers.ctrl {
                break;
            }

            // Global F-key navigation
            if let KeyCode::F(n) = event.key {
                let target = match n {
                    1 => {
                        // F1 = Help (contextual)
                        if panes.active().id() != "help" {
                            let current_id = panes.active().id();
                            let current_keymap = panes.active().keymap().clone();
                            let title = match current_id {
                                "rack" => "Rack",
                                "mixer" => "Mixer",
                                "server" => "Server",
                                "piano_roll" => "Piano Roll",
                                "sequencer" => "Sequencer",
                                "add" => "Add Module",
                                "edit" => "Edit Module",
                                _ => current_id,
                            };
                            if let Some(help) = panes.get_pane_mut::<HelpPane>("help") {
                                help.set_context(current_id, title, &current_keymap);
                            }
                            Some("help")
                        } else {
                            None
                        }
                    }
                    2 => Some("rack"),
                    3 => Some("piano_roll"),
                    4 => Some("sequencer"),
                    5 => Some("mixer"),
                    6 => Some("server"),
                    _ => None,
                };
                if let Some(id) = target {
                    panes.switch_to(id);
                    continue;
                }
            }

            let action = panes.handle_input(event);
            match &action {
                Action::Quit => break,
                Action::AddModule(_) => {
                    // Dispatch to rack pane and switch back
                    panes.dispatch_to("rack", &action);

                    // Rebuild routing to include new module
                    if audio_engine.is_running() {
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            let _ = audio_engine.rebuild_routing(rack_pane.rack());
                        }
                    }

                    panes.switch_to("rack");
                }
                Action::DeleteModule(module_id) => {
                    // Free synth first
                    if audio_engine.is_running() {
                        let _ = audio_engine.free_synth(*module_id);
                    }

                    // Remove from rack
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().remove_module(*module_id);
                    }

                    // Rebuild routing
                    if audio_engine.is_running() {
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            let _ = audio_engine.rebuild_routing(rack_pane.rack());
                        }
                    }
                }
                Action::EditModule(id) => {
                    // Get module data from rack pane
                    let module_data = panes
                        .get_pane_mut::<RackPane>("rack")
                        .and_then(|rack| rack.get_module_for_edit(*id));

                    if let Some((id, name, type_name, params)) = module_data {
                        // Set module data on edit pane and switch to it
                        if let Some(edit) = panes.get_pane_mut::<EditPane>("edit") {
                            edit.set_module(id, name, type_name, params);
                        }
                        panes.switch_to("edit");
                    }
                }
                Action::UpdateModuleParams(_, _) => {
                    // Dispatch to rack pane and switch back
                    panes.dispatch_to("rack", &action);
                    panes.switch_to("rack");
                }
                Action::SaveRack => {
                    let path = default_rack_path();
                    // Ensure parent directory exists
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        if let Err(e) = rack_pane.rack().save(&path) {
                            eprintln!("Failed to save rack: {}", e);
                        }
                    }
                }
                Action::LoadRack => {
                    let path = default_rack_path();
                    if path.exists() {
                        match RackState::load(&path) {
                            Ok(rack) => {
                                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                                    rack_pane.set_rack(rack);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to load rack: {}", e);
                            }
                        }
                    }
                }
                Action::AddConnection(_) | Action::RemoveConnection(_) => {
                    // Dispatch to rack pane
                    panes.dispatch_to("rack", &action);

                    // Rebuild audio routing when connections change
                    if audio_engine.is_running() {
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            let _ = audio_engine.rebuild_routing(rack_pane.rack());
                        }
                    }
                }
                Action::ConnectServer => {
                    let result = audio_engine.connect("127.0.0.1:57110");
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        match result {
                            Ok(()) => {
                                // Load synthdefs
                                let synthdef_dir = std::path::Path::new("synthdefs");
                                if let Err(e) = audio_engine.load_synthdefs(synthdef_dir) {
                                    server.set_status(
                                        audio::ServerStatus::Connected,
                                        &format!("Connected (synthdef warning: {})", e),
                                    );
                                } else {
                                    server.set_status(audio::ServerStatus::Connected, "Connected");
                                }
                            }
                            Err(e) => {
                                server.set_status(audio::ServerStatus::Error, &e.to_string())
                            }
                        }
                    }
                }
                Action::DisconnectServer => {
                    audio_engine.disconnect();
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio_engine.status(), "Disconnected");
                        server.set_server_running(audio_engine.server_running());
                    }
                }
                Action::StartServer => {
                    let result = audio_engine.start_server();
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        match result {
                            Ok(()) => {
                                server.set_status(audio::ServerStatus::Running, "Server started");
                                server.set_server_running(true);
                            }
                            Err(e) => {
                                server.set_status(audio::ServerStatus::Error, &e);
                                server.set_server_running(false);
                            }
                        }
                    }
                }
                Action::StopServer => {
                    audio_engine.stop_server();
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Stopped, "Server stopped");
                        server.set_server_running(false);
                    }
                }
                Action::CompileSynthDefs => {
                    let scd_path = std::path::Path::new("synthdefs/compile.scd");
                    match audio_engine.compile_synthdefs_async(scd_path) {
                        Ok(()) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(audio_engine.status(), "Compiling synthdefs...");
                            }
                        }
                        Err(e) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(audio_engine.status(), &e);
                            }
                        }
                    }
                }
                Action::LoadSynthDefs => {
                    let synthdef_dir = std::path::Path::new("synthdefs");
                    let result = audio_engine.load_synthdefs(synthdef_dir);
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        match result {
                            Ok(()) => server.set_status(audio_engine.status(), "Synthdefs loaded"),
                            Err(e) => server.set_status(audio_engine.status(), &e),
                        }
                    }
                }
                Action::SetModuleParam(module_id, ref param, value) => {
                    if audio_engine.is_running() {
                        let _ = audio_engine.set_param(*module_id, param, *value);
                    }
                }
                Action::MixerMove(delta) => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().mixer.move_selection(*delta);
                    }
                }
                Action::MixerJump(direction) => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().mixer.jump_selection(*direction);
                    }
                }
                Action::MixerAdjustLevel(delta) => {
                    // Collect audio updates, then apply (avoids borrow conflicts)
                    let mut updates: Vec<(u32, f32, bool)> = Vec::new();
                    let mut bus_update: Option<(u8, f32, bool, f32)> = None;
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let mixer = &mut rack_pane.rack_mut().mixer;
                        match mixer.selection {
                            MixerSelection::Channel(id) => {
                                if let Some(ch) = mixer.channel_mut(id) {
                                    ch.level = (ch.level + delta).clamp(0.0, 1.0);
                                }
                                updates = mixer.collect_channel_updates();
                            }
                            MixerSelection::Bus(id) => {
                                if let Some(bus) = mixer.bus_mut(id) {
                                    bus.level = (bus.level + delta).clamp(0.0, 1.0);
                                }
                                if let Some(bus) = mixer.bus(id) {
                                    let mute = mixer.effective_bus_mute(bus);
                                    bus_update = Some((id, bus.level, mute, bus.pan));
                                }
                            }
                            MixerSelection::Master => {
                                mixer.master_level = (mixer.master_level + delta).clamp(0.0, 1.0);
                                updates = mixer.collect_channel_updates();
                            }
                        }
                    }
                    if audio_engine.is_running() {
                        for (module_id, level, mute) in updates {
                            let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                        }
                        if let Some((bus_id, level, mute, pan)) = bus_update {
                            let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                        }
                    }
                }
                Action::MixerToggleMute => {
                    let mut updates: Vec<(u32, f32, bool)> = Vec::new();
                    let mut bus_update: Option<(u8, f32, bool, f32)> = None;
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let mixer = &mut rack_pane.rack_mut().mixer;
                        match mixer.selection {
                            MixerSelection::Channel(id) => {
                                if let Some(ch) = mixer.channel_mut(id) {
                                    ch.mute = !ch.mute;
                                }
                            }
                            MixerSelection::Bus(id) => {
                                if let Some(bus) = mixer.bus_mut(id) {
                                    bus.mute = !bus.mute;
                                }
                                if let Some(bus) = mixer.bus(id) {
                                    let mute = mixer.effective_bus_mute(bus);
                                    bus_update = Some((id, bus.level, mute, bus.pan));
                                }
                            }
                            MixerSelection::Master => {
                                mixer.master_mute = !mixer.master_mute;
                            }
                        }
                        updates = mixer.collect_channel_updates();
                    }
                    if audio_engine.is_running() {
                        for (module_id, level, mute) in updates {
                            let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                        }
                        if let Some((bus_id, level, mute, pan)) = bus_update {
                            let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                        }
                    }
                }
                Action::MixerToggleSolo => {
                    let mut updates: Vec<(u32, f32, bool)> = Vec::new();
                    let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let mixer = &mut rack_pane.rack_mut().mixer;
                        match mixer.selection {
                            MixerSelection::Channel(id) => {
                                if let Some(ch) = mixer.channel_mut(id) {
                                    ch.solo = !ch.solo;
                                }
                            }
                            MixerSelection::Bus(id) => {
                                if let Some(bus) = mixer.bus_mut(id) {
                                    bus.solo = !bus.solo;
                                }
                            }
                            MixerSelection::Master => {} // Master can't be soloed
                        }
                        // Solo affects ALL channels and buses
                        updates = mixer.collect_channel_updates();
                        for bus in &mixer.buses {
                            let mute = mixer.effective_bus_mute(bus);
                            bus_updates.push((bus.id, bus.level, mute, bus.pan));
                        }
                    }
                    if audio_engine.is_running() {
                        for (module_id, level, mute) in updates {
                            let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                        }
                        for (bus_id, level, mute, pan) in bus_updates {
                            let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                        }
                    }
                }
                Action::MixerCycleSection => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().mixer.cycle_section();
                    }
                }
                Action::MixerCycleOutput => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().mixer.cycle_output();
                    }
                }
                Action::MixerCycleOutputReverse => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        rack_pane.rack_mut().mixer.cycle_output_reverse();
                    }
                }
                Action::MixerAdjustSend(bus_id, delta) => {
                    let bus_id = *bus_id;
                    let delta = *delta;
                    let mut send_update: Option<(u8, u8, f32)> = None;
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let mixer = &mut rack_pane.rack_mut().mixer;
                        if let MixerSelection::Channel(ch_id) = mixer.selection {
                            if let Some(ch) = mixer.channel_mut(ch_id) {
                                if let Some(send) = ch.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                                    send.level = (send.level + delta).clamp(0.0, 1.0);
                                    send_update = Some((ch_id, bus_id, send.level));
                                }
                            }
                        }
                    }
                    if let Some((ch_id, bus_id, level)) = send_update {
                        if audio_engine.is_running() {
                            let _ = audio_engine.set_send_level(ch_id, bus_id, level);
                        }
                    }
                }
                Action::MixerToggleSend(bus_id) => {
                    let bus_id = *bus_id;
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let mixer = &mut rack_pane.rack_mut().mixer;
                        if let MixerSelection::Channel(ch_id) = mixer.selection {
                            if let Some(ch) = mixer.channel_mut(ch_id) {
                                if let Some(send) = ch.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                                    send.enabled = !send.enabled;
                                    // When toggling, set a default level if enabling with 0
                                    if send.enabled && send.level <= 0.0 {
                                        send.level = 0.5;
                                    }
                                }
                            }
                        }
                    }
                    // Sends being toggled requires a routing rebuild to create/remove send synths
                    if audio_engine.is_running() {
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            let _ = audio_engine.rebuild_routing(rack_pane.rack());
                        }
                    }
                }
                Action::PianoRollToggleNote => {
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        let pitch = pr_pane.cursor_pitch();
                        let tick = pr_pane.cursor_tick();
                        let dur = pr_pane.default_duration();
                        let vel = pr_pane.default_velocity();
                        let track = pr_pane.current_track();
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            rack_pane.rack_mut().piano_roll.toggle_note(track, pitch, tick, dur, vel);
                        }
                    }
                }
                Action::PianoRollAdjustDuration(delta) => {
                    let delta = *delta;
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        pr_pane.adjust_default_duration(delta);
                    }
                }
                Action::PianoRollAdjustVelocity(delta) => {
                    let delta = *delta;
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        pr_pane.adjust_default_velocity(delta);
                    }
                }
                Action::PianoRollPlayStop => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let pr = &mut rack_pane.rack_mut().piano_roll;
                        pr.playing = !pr.playing;
                        if !pr.playing {
                            pr.playhead = 0;
                            // Send immediate gate-off for all active notes
                            if audio_engine.is_running() {
                                for (module_id, _, _) in active_notes.drain(..) {
                                    let _ = audio_engine.send_note_off_bundled(module_id, 0.0);
                                }
                            }
                            active_notes.clear();
                        }
                    }
                }
                Action::PianoRollToggleLoop => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let pr = &mut rack_pane.rack_mut().piano_roll;
                        pr.looping = !pr.looping;
                    }
                }
                Action::PianoRollSetLoopStart => {
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        let tick = pr_pane.cursor_tick();
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            rack_pane.rack_mut().piano_roll.loop_start = tick;
                        }
                    }
                }
                Action::PianoRollSetLoopEnd => {
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        let tick = pr_pane.cursor_tick();
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            rack_pane.rack_mut().piano_roll.loop_end = tick;
                        }
                    }
                }
                Action::PianoRollChangeTrack(delta) => {
                    let delta = *delta;
                    let track_count = panes
                        .get_pane_mut::<RackPane>("rack")
                        .map(|r| r.rack().piano_roll.track_order.len())
                        .unwrap_or(0);
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        pr_pane.change_track(delta, track_count);
                    }
                }
                Action::PianoRollCycleTimeSig => {
                    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                        let pr = &mut rack_pane.rack_mut().piano_roll;
                        pr.time_signature = match pr.time_signature {
                            (4, 4) => (3, 4),
                            (3, 4) => (6, 8),
                            (6, 8) => (5, 4),
                            (5, 4) => (7, 8),
                            _ => (4, 4),
                        };
                    }
                }
                Action::PianoRollJump(_direction) => {
                    if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                        pr_pane.jump_to_end();
                    }
                }
                _ => {}
            }
        }

        // Poll for background compile completion
        if let Some(result) = audio_engine.poll_compile_result() {
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(msg) => server.set_status(audio_engine.status(), &msg),
                    Err(e) => server.set_status(audio_engine.status(), &e),
                }
            }
        }

        // Piano roll playback tick
        {
            let now = Instant::now();
            let elapsed = now.duration_since(last_frame_time);
            last_frame_time = now;

            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let pr = &mut rack_pane.rack_mut().piano_roll;
                if pr.playing {
                    // Calculate elapsed ticks: ticks = seconds * (bpm/60) * ticks_per_beat
                    let seconds = elapsed.as_secs_f32();
                    let ticks_f = seconds * (pr.bpm / 60.0) * pr.ticks_per_beat as f32;
                    let tick_delta = ticks_f as u32;

                    if tick_delta > 0 {
                        let old_playhead = pr.playhead;
                        pr.advance(tick_delta);
                        let new_playhead = pr.playhead;

                        // Determine tick range to scan for new notes
                        let (scan_start, scan_end) = if new_playhead >= old_playhead {
                            (old_playhead, new_playhead)
                        } else {
                            // Looped: scan from old to loop_end, then loop_start to new
                            // For simplicity, just scan loop_start..new_playhead after wrap
                            (pr.loop_start, new_playhead)
                        };

                        // Ticks-to-seconds factor for sub-frame timing
                        let secs_per_tick = 60.0 / (pr.bpm as f64 * pr.ticks_per_beat as f64);

                        // Find notes that start in this range across all tracks
                        let mut note_ons: Vec<(u32, u8, u8, u32, u32)> = Vec::new(); // (module_id, pitch, vel, duration, tick)
                        for &module_id in &pr.track_order {
                            if let Some(track) = pr.tracks.get(&module_id) {
                                for note in &track.notes {
                                    if note.tick >= scan_start && note.tick < scan_end {
                                        note_ons.push((module_id, note.pitch, note.velocity, note.duration, note.tick));
                                    }
                                }
                            }
                        }

                        // Send note-ons as timestamped bundles
                        if audio_engine.is_running() {
                            for (module_id, pitch, velocity, duration, note_tick) in &note_ons {
                                // Compute sub-frame offset: how far into the future this note should play
                                let ticks_from_now = if *note_tick >= old_playhead {
                                    (*note_tick - old_playhead) as f64
                                } else {
                                    0.0
                                };
                                let offset = ticks_from_now * secs_per_tick;
                                let vel_f = *velocity as f32 / 127.0;
                                let _ = audio_engine.send_note_on_bundled(*module_id, *pitch, vel_f, offset);
                                active_notes.push((*module_id, *pitch, *duration));
                            }
                        }

                        // Process active notes: decrement remaining ticks, send note-offs
                        let mut note_offs: Vec<(u32, u32)> = Vec::new(); // (module_id, remaining_ticks before this frame)
                        for note in active_notes.iter_mut() {
                            if note.2 <= tick_delta {
                                note_offs.push((note.0, note.2));
                                note.2 = 0;
                            } else {
                                note.2 -= tick_delta;
                            }
                        }
                        active_notes.retain(|n| n.2 > 0);

                        if audio_engine.is_running() {
                            for (module_id, remaining) in &note_offs {
                                let offset = *remaining as f64 * secs_per_tick;
                                let _ = audio_engine.send_note_off_bundled(*module_id, offset);
                            }
                        }
                    }
                }
            }
        }

        // Render
        let mut frame = backend.begin_frame()?;

        // Render the outer frame (border, header bar, console)
        app_frame.render(&mut frame);

        // Render pane content within the inner rect
        // Special handling for mixer pane which needs rack state
        let active_id = panes.active().id();
        if active_id == "mixer" {
            let rack_state = panes
                .get_pane_mut::<RackPane>("rack")
                .map(|r| r.rack().clone());
            if let Some(rack) = rack_state {
                if let Some(mixer_pane) = panes.get_pane_mut::<MixerPane>("mixer") {
                    mixer_pane.render_with_state(&mut frame, &rack);
                }
            }
        } else if active_id == "piano_roll" {
            let pr_state = panes
                .get_pane_mut::<RackPane>("rack")
                .map(|r| r.rack().piano_roll.clone());
            if let Some(pr) = pr_state {
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.render_with_state(&mut frame, &pr);
                }
            }
        } else {
            panes.render(&mut frame);
        }

        backend.end_frame(frame)?;
    }

    Ok(())
}
