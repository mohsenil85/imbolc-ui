use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use super::bus_allocator::BusAllocator;
use super::osc_client::OscClient;
use crate::state::{ModuleType, Param, ParamValue, PortDirection, PortType, RackState};

pub type ModuleId = u32;

// SuperCollider group IDs for execution ordering
pub const GROUP_SOURCES: i32 = 100;
pub const GROUP_PROCESSING: i32 = 200;
pub const GROUP_OUTPUT: i32 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Connected,
    Error,
}

/// Bus assignments for a single module
#[derive(Debug, Clone, Default)]
pub struct BusAssignment {
    /// Audio output bus (for modules with audio output)
    pub audio_out: Option<i32>,
    /// Audio input bus (for modules reading audio)
    pub audio_in: Option<i32>,
    /// Control/gate input buses: port_name -> bus_index
    pub control_ins: HashMap<String, i32>,
    /// Control/gate output buses: port_name -> bus_index
    pub control_outs: HashMap<String, i32>,
}

/// Maximum simultaneous voices per MIDI module
const MAX_VOICES_PER_MODULE: usize = 16;

/// A single mono voice (Midi-only node, shared control buses)
#[derive(Debug, Clone)]
pub struct VoiceEntry {
    pub module_id: ModuleId,
    pub pitch: u8,
    pub node_id: i32,
}

/// Template describing a MIDI module's downstream signal chain
#[derive(Debug, Clone)]
pub struct ChainTemplate {
    /// Ordered list of (module_id, module_type) downstream from Midi, excluding Output
    pub modules: Vec<(ModuleId, ModuleType)>,
    /// The audio bus that the Output module reads from (where voices sum)
    pub output_audio_bus: i32,
}

/// A polyphonic voice chain: entire signal chain spawned per note
#[derive(Debug, Clone)]
pub struct VoiceChain {
    pub midi_module_id: ModuleId,
    pub pitch: u8,
    pub group_id: i32,
    pub midi_node_id: i32,
}

pub struct AudioEngine {
    client: Option<OscClient>,
    node_map: HashMap<ModuleId, i32>,
    next_node_id: i32,
    is_running: bool,
    scsynth_process: Option<Child>,
    server_status: ServerStatus,
    compile_receiver: Option<Receiver<Result<String, String>>>,
    is_compiling: bool,
    bus_allocator: BusAllocator,
    groups_created: bool,
    /// Dedicated audio bus per mixer bus (bus_id -> SC audio bus index)
    bus_audio_buses: HashMap<u8, i32>,
    /// Send synth nodes: (channel_id, bus_id) -> node_id
    send_node_map: HashMap<(u8, u8), i32>,
    /// Bus output synth nodes: bus_id -> node_id
    bus_node_map: HashMap<u8, i32>,
    /// Bus assignments for MIDI modules (needed for voice spawning)
    midi_bus_assignments: HashMap<ModuleId, BusAssignment>,
    /// Active mono voices (Midi-only nodes)
    voice_list: Vec<VoiceEntry>,
    /// Active poly voice chains (full signal chain per note)
    voice_chains: Vec<VoiceChain>,
    /// Chain templates for poly mode: midi_module_id -> template
    chain_templates: HashMap<ModuleId, ChainTemplate>,
    /// Set of module IDs that are part of a poly chain (skip static synth creation)
    poly_chain_modules: std::collections::HashSet<ModuleId>,
    /// Next available voice bus (audio) — starts after static allocation
    next_voice_audio_bus: i32,
    /// Next available voice bus (control) — starts after static allocation
    next_voice_control_bus: i32,
    /// Next group ID for voice groups
    next_group_id: i32,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            client: None,
            node_map: HashMap::new(),
            next_node_id: 1000,
            is_running: false,
            scsynth_process: None,
            server_status: ServerStatus::Stopped,
            compile_receiver: None,
            is_compiling: false,
            bus_allocator: BusAllocator::new(),
            groups_created: false,
            bus_audio_buses: HashMap::new(),
            send_node_map: HashMap::new(),
            bus_node_map: HashMap::new(),
            midi_bus_assignments: HashMap::new(),
            voice_list: Vec::new(),
            voice_chains: Vec::new(),
            chain_templates: HashMap::new(),
            poly_chain_modules: std::collections::HashSet::new(),
            next_voice_audio_bus: 16,
            next_voice_control_bus: 0,
            next_group_id: 1000,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn status(&self) -> ServerStatus {
        self.server_status
    }

    pub fn server_running(&self) -> bool {
        self.scsynth_process.is_some()
    }

    pub fn is_compiling(&self) -> bool {
        self.is_compiling
    }

    /// Start the scsynth server process
    pub fn start_server(&mut self) -> Result<(), String> {
        if self.scsynth_process.is_some() {
            return Err("Server already running".to_string());
        }

        self.server_status = ServerStatus::Starting;

        // Try to find scsynth in common locations
        let scsynth_paths = [
            "scsynth",
            "/Applications/SuperCollider.app/Contents/Resources/scsynth",
            "/usr/local/bin/scsynth",
            "/usr/bin/scsynth",
        ];

        let mut child = None;
        for path in &scsynth_paths {
            match Command::new(path)
                .args(["-u", "57110"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => {
                    child = Some(c);
                    break;
                }
                Err(_) => continue,
            }
        }

        match child {
            Some(c) => {
                self.scsynth_process = Some(c);
                self.server_status = ServerStatus::Running;
                // Give server time to start
                thread::sleep(Duration::from_millis(500));
                Ok(())
            }
            None => {
                self.server_status = ServerStatus::Error;
                Err("Could not find scsynth. Install SuperCollider.".to_string())
            }
        }
    }

    /// Stop the scsynth server process
    pub fn stop_server(&mut self) {
        // Disconnect first
        self.disconnect();

        if let Some(mut child) = self.scsynth_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.server_status = ServerStatus::Stopped;
    }

    /// Start compiling synthdefs in background thread
    pub fn compile_synthdefs_async(&mut self, scd_path: &Path) -> Result<(), String> {
        if self.is_compiling {
            return Err("Compilation already in progress".to_string());
        }

        if !scd_path.exists() {
            return Err(format!("File not found: {}", scd_path.display()));
        }

        let path = scd_path.to_path_buf();
        let (tx, rx) = mpsc::channel();
        self.compile_receiver = Some(rx);
        self.is_compiling = true;

        thread::spawn(move || {
            let result = Self::run_sclang(&path);
            let _ = tx.send(result);
        });

        Ok(())
    }

    /// Poll for compilation result (non-blocking)
    pub fn poll_compile_result(&mut self) -> Option<Result<String, String>> {
        if let Some(ref rx) = self.compile_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.compile_receiver = None;
                    self.is_compiling = false;
                    Some(result)
                }
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.compile_receiver = None;
                    self.is_compiling = false;
                    Some(Err("Compilation thread terminated unexpectedly".to_string()))
                }
            }
        } else {
            None
        }
    }

    /// Run sclang synchronously (called from background thread)
    fn run_sclang(scd_path: &PathBuf) -> Result<String, String> {
        // Try to find sclang in common locations
        let sclang_paths = [
            "sclang",
            "/Applications/SuperCollider.app/Contents/MacOS/sclang",
            "/usr/local/bin/sclang",
            "/usr/bin/sclang",
        ];

        for path in &sclang_paths {
            match Command::new(path).arg(scd_path).output() {
                Ok(output) => {
                    if output.status.success() {
                        return Ok("Synthdefs compiled successfully".to_string());
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(format!("Compilation failed: {}", stderr));
                    }
                }
                Err(_) => continue,
            }
        }

        Err("Could not find sclang. Install SuperCollider.".to_string())
    }

    pub fn connect(&mut self, server_addr: &str) -> std::io::Result<()> {
        let client = OscClient::new(server_addr)?;
        self.client = Some(client);
        self.is_running = true;
        self.server_status = ServerStatus::Connected;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(ref client) = self.client {
            for &node_id in self.node_map.values() {
                let _ = client.free_node(node_id);
            }
        }
        self.node_map.clear();
        self.send_node_map.clear();
        self.bus_node_map.clear();
        self.bus_audio_buses.clear();
        self.midi_bus_assignments.clear();
        self.voice_list.clear();
        self.voice_chains.clear();
        self.chain_templates.clear();
        self.poly_chain_modules.clear();
        self.bus_allocator.reset();
        self.groups_created = false;
        self.client = None;
        self.is_running = false;
        // Keep server_status as Running if scsynth is still running
        if self.scsynth_process.is_some() {
            self.server_status = ServerStatus::Running;
        } else {
            self.server_status = ServerStatus::Stopped;
        }
    }

    /// Create the 3 execution-order groups (sources → processing → output)
    fn ensure_groups(&mut self) -> Result<(), String> {
        if self.groups_created {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;
        // addToTail(1) of default group(0)
        client.create_group(GROUP_SOURCES, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_PROCESSING, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_OUTPUT, 1, 0).map_err(|e| e.to_string())?;
        self.groups_created = true;
        Ok(())
    }

    /// Determine which group a module type belongs to
    fn group_for_module(module_type: ModuleType) -> i32 {
        match module_type {
            ModuleType::Midi | ModuleType::SawOsc | ModuleType::SinOsc
            | ModuleType::SqrOsc | ModuleType::TriOsc | ModuleType::Lfo
            | ModuleType::AdsrEnv => GROUP_SOURCES,
            ModuleType::Lpf | ModuleType::Hpf | ModuleType::Bpf
            | ModuleType::Delay | ModuleType::Reverb => GROUP_PROCESSING,
            ModuleType::Output => GROUP_OUTPUT,
        }
    }

    fn synth_def_name(module_type: ModuleType) -> &'static str {
        match module_type {
            ModuleType::Midi => "tuidaw_midi",
            ModuleType::SawOsc => "tuidaw_saw",
            ModuleType::SinOsc => "tuidaw_sin",
            ModuleType::SqrOsc => "tuidaw_sqr",
            ModuleType::TriOsc => "tuidaw_tri",
            ModuleType::Lpf => "tuidaw_lpf",
            ModuleType::Hpf => "tuidaw_hpf",
            ModuleType::Bpf => "tuidaw_bpf",
            ModuleType::AdsrEnv => "tuidaw_adsr",
            ModuleType::Lfo => "tuidaw_lfo",
            ModuleType::Delay => "tuidaw_delay",
            ModuleType::Reverb => "tuidaw_reverb",
            ModuleType::Output => "tuidaw_output",
        }
    }

    /// Resolve bus routing for all modules based on connections
    fn resolve_routing(&mut self, rack: &RackState) -> HashMap<ModuleId, BusAssignment> {
        self.bus_allocator.reset();
        let mut assignments: HashMap<ModuleId, BusAssignment> = HashMap::new();

        // First pass: allocate output buses for all modules with outputs
        for &module_id in &rack.order {
            if let Some(module) = rack.modules.get(&module_id) {
                let mut assignment = BusAssignment::default();
                let ports = module.module_type.ports();

                for port in &ports {
                    if port.direction == PortDirection::Output {
                        match port.port_type {
                            PortType::Audio => {
                                let bus = self.bus_allocator.get_or_alloc_audio_bus(module_id, port.name);
                                assignment.audio_out = Some(bus);
                            }
                            PortType::Control | PortType::Gate => {
                                let bus = self.bus_allocator.get_or_alloc_control_bus(module_id, port.name);
                                assignment.control_outs.insert(port.name.to_string(), bus);
                            }
                        }
                    }
                }

                assignments.insert(module_id, assignment);
            }
        }

        // Second pass: resolve input buses based on connections
        for connection in &rack.connections {
            let src_module_id = connection.src.module_id;
            let dst_module_id = connection.dst.module_id;
            let src_port = &connection.src.port_name;
            let dst_port = &connection.dst.port_name;

            // Get source module's port type
            let src_port_type = rack.modules.get(&src_module_id)
                .and_then(|m| m.module_type.ports().into_iter().find(|p| p.name == src_port))
                .map(|p| p.port_type);

            if let Some(port_type) = src_port_type {
                match port_type {
                    PortType::Audio => {
                        // Get source's output bus
                        if let Some(src_assignment) = assignments.get(&src_module_id) {
                            if let Some(bus) = src_assignment.audio_out {
                                // Set destination's input to same bus
                                if let Some(dst_assignment) = assignments.get_mut(&dst_module_id) {
                                    dst_assignment.audio_in = Some(bus);
                                }
                            }
                        }
                    }
                    PortType::Control | PortType::Gate => {
                        // Get source's control output bus
                        if let Some(src_assignment) = assignments.get(&src_module_id) {
                            if let Some(&bus) = src_assignment.control_outs.get(src_port) {
                                // Set destination's control input to same bus
                                if let Some(dst_assignment) = assignments.get_mut(&dst_module_id) {
                                    dst_assignment.control_ins.insert(dst_port.to_string(), bus);
                                }
                            }
                        }
                    }
                }
            }
        }

        assignments
    }

    /// Topologically sort modules based on connections (sources before destinations)
    fn topological_sort(rack: &RackState) -> Vec<ModuleId> {
        use std::collections::HashSet;

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_marks = HashSet::new();

        fn visit(
            module_id: ModuleId,
            rack: &RackState,
            visited: &mut HashSet<ModuleId>,
            temp_marks: &mut HashSet<ModuleId>,
            result: &mut Vec<ModuleId>,
        ) {
            if visited.contains(&module_id) {
                return;
            }
            if temp_marks.contains(&module_id) {
                // Cycle detected, just skip
                return;
            }

            temp_marks.insert(module_id);

            // Visit all modules that feed into this one
            for conn in rack.connections.iter() {
                if conn.dst.module_id == module_id {
                    visit(conn.src.module_id, rack, visited, temp_marks, result);
                }
            }

            temp_marks.remove(&module_id);
            visited.insert(module_id);
            result.push(module_id);
        }

        for &module_id in &rack.order {
            visit(module_id, rack, &mut visited, &mut temp_marks, &mut result);
        }

        result
    }

    /// Create a synth with bus assignments
    fn create_synth_with_routing(
        &mut self,
        module_id: ModuleId,
        module_type: ModuleType,
        params: &[Param],
        bus_assignment: &BusAssignment,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        // Start with module parameters
        let mut param_pairs: Vec<(String, f32)> = params
            .iter()
            .filter_map(|p| match &p.value {
                ParamValue::Float(v) => Some((p.name.clone(), *v)),
                ParamValue::Int(v) => Some((p.name.clone(), *v as f32)),
                ParamValue::Bool(v) => Some((p.name.clone(), if *v { 1.0 } else { 0.0 })),
            })
            .collect();

        // Add bus routing parameters
        if let Some(audio_out) = bus_assignment.audio_out {
            param_pairs.push(("out".to_string(), audio_out as f32));
        }

        if let Some(audio_in) = bus_assignment.audio_in {
            param_pairs.push(("in".to_string(), audio_in as f32));
        }

        // Control outputs (for MIDI, LFO, ADSR)
        for (port_name, bus) in &bus_assignment.control_outs {
            let param_name = format!("{}_out", port_name);
            param_pairs.push((param_name, *bus as f32));
        }

        // Control inputs (for oscillators, filters, etc.)
        for (port_name, bus) in &bus_assignment.control_ins {
            let param_name = format!("{}_in", port_name);
            param_pairs.push((param_name, *bus as f32));
        }

        let group = Self::group_for_module(module_type);
        client
            .create_synth_in_group(Self::synth_def_name(module_type), node_id, group, &param_pairs)
            .map_err(|e| e.to_string())?;

        self.node_map.insert(module_id, node_id);
        Ok(())
    }

    /// Rebuild all routing - frees all synths, reallocates buses, recreates in order
    pub fn rebuild_routing(&mut self, rack: &RackState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }

        // Ensure groups exist
        self.ensure_groups()?;

        // Free all existing synths (modules, sends, bus outputs) and voices
        if let Some(ref client) = self.client {
            for &node_id in self.node_map.values() {
                let _ = client.free_node(node_id);
            }
            for &node_id in self.send_node_map.values() {
                let _ = client.free_node(node_id);
            }
            for &node_id in self.bus_node_map.values() {
                let _ = client.free_node(node_id);
            }
            for voice in self.voice_list.drain(..) {
                let _ = client.free_node(voice.node_id);
            }
            for chain in self.voice_chains.drain(..) {
                let _ = client.free_node(chain.group_id);
            }
        }
        self.node_map.clear();
        self.send_node_map.clear();
        self.bus_node_map.clear();
        self.bus_audio_buses.clear();
        self.midi_bus_assignments.clear();
        self.chain_templates.clear();
        self.poly_chain_modules.clear();

        // Resolve routing
        let assignments = self.resolve_routing(rack);

        // Get topologically sorted order
        let sorted_modules = Self::topological_sort(rack);

        // Build chain templates for poly MIDI modules:
        // Walk downstream from each Midi module, collecting modules until Output.
        for &module_id in &rack.order {
            if let Some(module) = rack.modules.get(&module_id) {
                if module.module_type != ModuleType::Midi {
                    continue;
                }
                // Check if this MIDI module's track is polyphonic
                let is_poly = rack.piano_roll.tracks
                    .get(&module_id)
                    .map_or(true, |t| t.polyphonic);
                if !is_poly {
                    continue;
                }

                // Walk the connection graph downstream from this Midi module
                let mut chain_modules: Vec<(ModuleId, ModuleType)> = Vec::new();
                let mut output_audio_bus: Option<i32> = None;
                let mut current_id = module_id;

                loop {
                    // Find the downstream module connected to current_id's output
                    let next = rack.connections.iter().find(|c| {
                        c.src.module_id == current_id
                    });
                    match next {
                        Some(conn) => {
                            let dst_id = conn.dst.module_id;
                            if let Some(dst_mod) = rack.modules.get(&dst_id) {
                                if dst_mod.module_type == ModuleType::Output {
                                    // Found the Output — record its audio_in bus
                                    output_audio_bus = assignments
                                        .get(&dst_id)
                                        .and_then(|a| a.audio_in);
                                    break;
                                } else {
                                    chain_modules.push((dst_id, dst_mod.module_type));
                                    current_id = dst_id;
                                }
                            } else {
                                break;
                            }
                        }
                        None => break,
                    }
                }

                if let Some(bus) = output_audio_bus {
                    // Mark all chain modules as poly (skip static synth creation)
                    for &(mid, _) in &chain_modules {
                        self.poly_chain_modules.insert(mid);
                    }
                    self.chain_templates.insert(module_id, ChainTemplate {
                        modules: chain_modules,
                        output_audio_bus: bus,
                    });
                }
            }
        }

        // Store bus allocator state for voice bus allocation
        self.next_voice_audio_bus = self.bus_allocator.next_audio_bus;
        self.next_voice_control_bus = self.bus_allocator.next_control_bus;

        // Create synths in order (skip Midi — voices are spawned dynamically)
        // Also skip modules that are part of a poly chain (they get cloned per voice)
        for module_id in sorted_modules {
            if let Some(module) = rack.modules.get(&module_id) {
                let assignment = assignments.get(&module_id).cloned().unwrap_or_default();
                if module.module_type == ModuleType::Midi {
                    // Store bus assignments for mono voice spawning
                    self.midi_bus_assignments.insert(module_id, assignment);
                } else if self.poly_chain_modules.contains(&module_id) {
                    // Skip — will be cloned per voice in spawn_voice_chain
                } else {
                    self.create_synth_with_routing(
                        module_id,
                        module.module_type,
                        &module.params,
                        &assignment,
                    )?;
                }
            }
        }

        // Allocate audio buses for each mixer bus
        // Start at a high bus index to avoid collisions with module buses
        let bus_audio_base = 200;
        for bus in &rack.mixer.buses {
            self.bus_audio_buses.insert(bus.id, bus_audio_base + (bus.id as i32 - 1) * 2);
        }

        // Create send synths for channels with enabled sends
        for ch in &rack.mixer.channels {
            if ch.module_id.is_none() {
                continue;
            }
            // Get the channel's output audio bus from the assigned module
            let ch_audio_bus = ch.module_id
                .and_then(|mid| assignments.get(&mid))
                .and_then(|a| a.audio_in) // Output module reads from this bus
                .unwrap_or(16); // fallback

            for send in &ch.sends {
                if !send.enabled || send.level <= 0.0 {
                    continue;
                }
                if let Some(&bus_audio) = self.bus_audio_buses.get(&send.bus_id) {
                    let node_id = self.next_node_id;
                    self.next_node_id += 1;
                    let params = vec![
                        ("in".to_string(), ch_audio_bus as f32),
                        ("out".to_string(), bus_audio as f32),
                        ("level".to_string(), send.level),
                    ];
                    if let Some(ref client) = self.client {
                        client
                            .create_synth_in_group("tuidaw_send", node_id, GROUP_OUTPUT, &params)
                            .map_err(|e| e.to_string())?;
                    }
                    self.send_node_map.insert((ch.id, send.bus_id), node_id);
                }
            }
        }

        // Create bus output synths
        for bus in &rack.mixer.buses {
            if let Some(&bus_audio) = self.bus_audio_buses.get(&bus.id) {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let mute = rack.mixer.effective_bus_mute(bus);
                let params = vec![
                    ("in".to_string(), bus_audio as f32),
                    ("level".to_string(), bus.level),
                    ("mute".to_string(), if mute { 1.0 } else { 0.0 }),
                    ("pan".to_string(), bus.pan),
                ];
                if let Some(ref client) = self.client {
                    client
                        .create_synth_in_group("tuidaw_bus_out", node_id, GROUP_OUTPUT, &params)
                        .map_err(|e| e.to_string())?;
                }
                self.bus_node_map.insert(bus.id, node_id);
            }
        }

        Ok(())
    }

    /// Set send level for a channel->bus send in real-time
    pub fn set_send_level(&self, channel_id: u8, bus_id: u8, level: f32) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let node_id = self.send_node_map
            .get(&(channel_id, bus_id))
            .ok_or_else(|| format!("No send node for ch{} -> bus{}", channel_id, bus_id))?;
        client.set_param(*node_id, "level", level).map_err(|e| e.to_string())
    }

    /// Set bus output mixer params (level, mute, pan) in real-time
    pub fn set_bus_mixer_params(&self, bus_id: u8, level: f32, mute: bool, pan: f32) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let node_id = self.bus_node_map
            .get(&bus_id)
            .ok_or_else(|| format!("No bus output node for bus{}", bus_id))?;
        client.set_param(*node_id, "level", level).map_err(|e| e.to_string())?;
        client.set_param(*node_id, "mute", if mute { 1.0 } else { 0.0 }).map_err(|e| e.to_string())?;
        client.set_param(*node_id, "pan", pan).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn create_synth(
        &mut self,
        module_id: ModuleId,
        module_type: ModuleType,
        params: &[Param],
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        let param_pairs: Vec<(String, f32)> = params
            .iter()
            .filter_map(|p| match &p.value {
                ParamValue::Float(v) => Some((p.name.clone(), *v)),
                ParamValue::Int(v) => Some((p.name.clone(), *v as f32)),
                ParamValue::Bool(v) => Some((p.name.clone(), if *v { 1.0 } else { 0.0 })),
            })
            .collect();

        client
            .create_synth(Self::synth_def_name(module_type), node_id, &param_pairs)
            .map_err(|e| e.to_string())?;

        self.node_map.insert(module_id, node_id);
        Ok(())
    }

    pub fn free_synth(&mut self, module_id: ModuleId) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        if let Some(node_id) = self.node_map.remove(&module_id) {
            client.free_node(node_id).map_err(|e| e.to_string())?;
        }
        self.bus_allocator.free_module_buses(module_id);
        Ok(())
    }

    pub fn set_param(&self, module_id: ModuleId, param: &str, value: f32) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let node_id = self
            .node_map
            .get(&module_id)
            .ok_or_else(|| format!("No synth for module {}", module_id))?;
        client
            .set_param(*node_id, param, value)
            .map_err(|e| e.to_string())
    }

    /// Set mixer params on an Output module
    pub fn set_output_mixer_params(
        &self,
        module_id: ModuleId,
        level: f32,
        mute: bool,
    ) -> Result<(), String> {
        self.set_param(module_id, "level", level)?;
        self.set_param(module_id, "mute", if mute { 1.0 } else { 0.0 })?;
        Ok(())
    }

    /// Spawn a voice — dispatches to mono or poly based on flag
    pub fn spawn_voice(
        &mut self,
        module_id: ModuleId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        polyphonic: bool,
        rack: &RackState,
    ) -> Result<(), String> {
        if polyphonic && self.chain_templates.contains_key(&module_id) {
            self.spawn_voice_chain(module_id, pitch, velocity, offset_secs, rack)
        } else {
            self.spawn_mono_voice(module_id, pitch, velocity, offset_secs)
        }
    }

    /// Spawn a mono voice (Midi-only node, shared control buses) — original behavior
    fn spawn_mono_voice(
        &mut self,
        module_id: ModuleId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let bus_assignment = self
            .midi_bus_assignments
            .get(&module_id)
            .cloned()
            .ok_or_else(|| format!("No bus assignment for MIDI module {}", module_id))?;

        // Voice-steal: if at limit for this module, release oldest voice
        let count = self.voice_list.iter().filter(|v| v.module_id == module_id).count();
        if count >= MAX_VOICES_PER_MODULE {
            if let Some(pos) = self.voice_list.iter().position(|v| v.module_id == module_id) {
                let old = self.voice_list.remove(pos);
                let _ = client.set_param(old.node_id, "gate", 0.0);
            }
        }

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        let freq = 440.0 * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        // Build /s_new params: note, freq, vel, gate=1, plus bus routing
        let mut params: Vec<(String, f32)> = vec![
            ("note".to_string(), pitch as f32),
            ("freq".to_string(), freq as f32),
            ("vel".to_string(), velocity),
            ("gate".to_string(), 1.0),
        ];

        // Add bus routing from stored assignment
        for (port_name, bus) in &bus_assignment.control_outs {
            params.push((format!("{}_out", port_name), *bus as f32));
        }

        // Build the /s_new message
        let group = Self::group_for_module(ModuleType::Midi);
        let mut args: Vec<rosc::OscType> = vec![
            rosc::OscType::String(Self::synth_def_name(ModuleType::Midi).to_string()),
            rosc::OscType::Int(node_id),
            rosc::OscType::Int(1), // addToTail
            rosc::OscType::Int(group),
        ];
        for (name, value) in &params {
            args.push(rosc::OscType::String(name.clone()));
            args.push(rosc::OscType::Float(*value));
        }

        let msg = rosc::OscMessage {
            addr: "/s_new".to_string(),
            args,
        };

        let time = super::osc_client::osc_time_from_now(offset_secs);
        client
            .send_bundle(vec![msg], time)
            .map_err(|e| e.to_string())?;

        self.voice_list.push(VoiceEntry {
            module_id,
            pitch,
            node_id,
        });

        Ok(())
    }

    /// Spawn a full signal chain per voice (poly mode)
    fn spawn_voice_chain(
        &mut self,
        midi_module_id: ModuleId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        rack: &RackState,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let template = self.chain_templates.get(&midi_module_id)
            .ok_or_else(|| format!("No chain template for MIDI module {}", midi_module_id))?
            .clone();

        // Voice-steal: if at limit for this module, free oldest chain group
        let count = self.voice_chains.iter().filter(|v| v.midi_module_id == midi_module_id).count();
        if count >= MAX_VOICES_PER_MODULE {
            if let Some(pos) = self.voice_chains.iter().position(|v| v.midi_module_id == midi_module_id) {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

        // Create a group for this voice chain
        let group_id = self.next_group_id;
        self.next_group_id += 1;

        // Allocate per-voice control buses (freq, gate, vel)
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        // Allocate per-voice audio buses for inter-module connections
        let mut voice_audio_buses: Vec<i32> = Vec::new();
        for _ in 0..template.modules.len() {
            let bus = self.next_voice_audio_bus;
            self.next_voice_audio_bus += 2; // stereo
            voice_audio_buses.push(bus);
        }

        let freq = 440.0 * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        let mut messages: Vec<rosc::OscMessage> = Vec::new();

        // 1. Create group: /g_new group_id addToTail GROUP_SOURCES
        messages.push(rosc::OscMessage {
            addr: "/g_new".to_string(),
            args: vec![
                rosc::OscType::Int(group_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(GROUP_SOURCES),
            ],
        });

        // 2. MIDI node
        let midi_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String(Self::synth_def_name(ModuleType::Midi).to_string()),
                rosc::OscType::Int(midi_node_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(group_id),
            ];
            let params: Vec<(String, f32)> = vec![
                ("note".to_string(), pitch as f32),
                ("freq".to_string(), freq as f32),
                ("vel".to_string(), velocity),
                ("gate".to_string(), 1.0),
                ("freq_out".to_string(), voice_freq_bus as f32),
                ("gate_out".to_string(), voice_gate_bus as f32),
                ("vel_out".to_string(), voice_vel_bus as f32),
            ];
            for (name, value) in &params {
                args.push(rosc::OscType::String(name.clone()));
                args.push(rosc::OscType::Float(*value));
            }
            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // 3. Chain modules (osc, filter, etc.)
        for (i, &(mod_id, mod_type)) in template.modules.iter().enumerate() {
            let node_id = self.next_node_id;
            self.next_node_id += 1;

            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String(Self::synth_def_name(mod_type).to_string()),
                rosc::OscType::Int(node_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(group_id),
            ];

            // Snapshot params from the rack module
            if let Some(module) = rack.modules.get(&mod_id) {
                for p in &module.params {
                    let val = match &p.value {
                        ParamValue::Float(v) => *v,
                        ParamValue::Int(v) => *v as f32,
                        ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                    };
                    args.push(rosc::OscType::String(p.name.clone()));
                    args.push(rosc::OscType::Float(val));
                }
            }

            // Wire control inputs (freq, gate, vel) from per-voice buses
            let ports = mod_type.ports();
            for port in &ports {
                if port.direction == crate::state::PortDirection::Input {
                    match port.name {
                        "freq" => {
                            args.push(rosc::OscType::String("freq_in".to_string()));
                            args.push(rosc::OscType::Float(voice_freq_bus as f32));
                        }
                        "gate" => {
                            args.push(rosc::OscType::String("gate_in".to_string()));
                            args.push(rosc::OscType::Float(voice_gate_bus as f32));
                        }
                        "vel" => {
                            args.push(rosc::OscType::String("vel_in".to_string()));
                            args.push(rosc::OscType::Float(voice_vel_bus as f32));
                        }
                        _ => {}
                    }
                }
            }

            // Wire audio input from previous module's audio output bus
            let has_audio_in = ports.iter().any(|p| p.name == "in" && p.port_type == crate::state::PortType::Audio && p.direction == crate::state::PortDirection::Input);
            if has_audio_in && i > 0 {
                args.push(rosc::OscType::String("in".to_string()));
                args.push(rosc::OscType::Float(voice_audio_buses[i - 1] as f32));
            }

            // Wire audio output
            let has_audio_out = ports.iter().any(|p| p.name == "out" && p.port_type == crate::state::PortType::Audio && p.direction == crate::state::PortDirection::Output);
            if has_audio_out {
                // Last module in chain outputs to the Output module's bus; otherwise to next voice audio bus
                let out_bus = if i == template.modules.len() - 1 {
                    template.output_audio_bus
                } else {
                    voice_audio_buses[i]
                };
                args.push(rosc::OscType::String("out".to_string()));
                args.push(rosc::OscType::Float(out_bus as f32));
            }

            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // Send all as one timed bundle
        let time = super::osc_client::osc_time_from_now(offset_secs);
        client
            .send_bundle(messages, time)
            .map_err(|e| e.to_string())?;

        self.voice_chains.push(VoiceChain {
            midi_module_id,
            pitch,
            group_id,
            midi_node_id,
        });

        Ok(())
    }

    /// Release a specific voice by module and pitch (note-off)
    pub fn release_voice(
        &mut self,
        module_id: ModuleId,
        pitch: u8,
        offset_secs: f64,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        // Check poly voice chains first
        if let Some(pos) = self
            .voice_chains
            .iter()
            .position(|v| v.midi_module_id == module_id && v.pitch == pitch)
        {
            let chain = self.voice_chains.remove(pos);
            // Send gate=0 to the midi node
            let time = super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(chain.midi_node_id, &[("gate", 0.0)], time)
                .map_err(|e| e.to_string())?;
            // Schedule group free after 5 seconds (release envelope time)
            let cleanup_time = super::osc_client::osc_time_from_now(offset_secs + 5.0);
            client
                .send_bundle(
                    vec![rosc::OscMessage {
                        addr: "/n_free".to_string(),
                        args: vec![rosc::OscType::Int(chain.group_id)],
                    }],
                    cleanup_time,
                )
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Fall back to mono voice list
        if let Some(pos) = self
            .voice_list
            .iter()
            .position(|v| v.module_id == module_id && v.pitch == pitch)
        {
            let voice = self.voice_list.remove(pos);
            let time = super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(voice.node_id, &[("gate", 0.0)], time)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Release all active voices (e.g. on playback stop)
    pub fn release_all_voices(&mut self) {
        if let Some(ref client) = self.client {
            for voice in self.voice_list.drain(..) {
                let _ = client.set_param(voice.node_id, "gate", 0.0);
            }
            for chain in self.voice_chains.drain(..) {
                let _ = client.free_node(chain.group_id);
            }
        }
    }

    pub fn load_synthdefs(&self, dir: &Path) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map_or(false, |e| e == "scsyndef") {
                let data = fs::read(&path).map_err(|e| e.to_string())?;
                client
                    .send_message("/d_recv", vec![rosc::OscType::Blob(data)])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_server();
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}
