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
        self.bus_allocator.reset();
        self.client = None;
        self.is_running = false;
        // Keep server_status as Running if scsynth is still running
        if self.scsynth_process.is_some() {
            self.server_status = ServerStatus::Running;
        } else {
            self.server_status = ServerStatus::Stopped;
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

        client
            .create_synth(Self::synth_def_name(module_type), node_id, &param_pairs)
            .map_err(|e| e.to_string())?;

        self.node_map.insert(module_id, node_id);
        Ok(())
    }

    /// Rebuild all routing - frees all synths, reallocates buses, recreates in order
    pub fn rebuild_routing(&mut self, rack: &RackState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }

        // Free all existing synths
        if let Some(ref client) = self.client {
            for &node_id in self.node_map.values() {
                let _ = client.free_node(node_id);
            }
        }
        self.node_map.clear();

        // Resolve routing
        let assignments = self.resolve_routing(rack);

        // Get topologically sorted order
        let sorted_modules = Self::topological_sort(rack);

        // Create synths in order
        for module_id in sorted_modules {
            if let Some(module) = rack.modules.get(&module_id) {
                let assignment = assignments.get(&module_id).cloned().unwrap_or_default();
                self.create_synth_with_routing(
                    module_id,
                    module.module_type,
                    &module.params,
                    &assignment,
                )?;
            }
        }

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
