use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

/// State for an active disk recording session
struct RecordingState {
    bufnum: i32,
    node_id: i32,
    path: PathBuf,
    started_at: Instant,
}

use super::bus_allocator::BusAllocator;
use super::osc_client::{OscClient, osc_time_immediate};
use crate::state::{AutomationTarget, BufferId, CustomSynthDefRegistry, EffectType, FilterType, SourceType, ParamValue, SessionState, InstrumentId, InstrumentState};

#[allow(dead_code)]
pub type ModuleId = u32;

// SuperCollider group IDs for execution ordering
pub const GROUP_SOURCES: i32 = 100;
pub const GROUP_PROCESSING: i32 = 200;
pub const GROUP_OUTPUT: i32 = 300;
pub const GROUP_RECORD: i32 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Connected,
    Error,
}

/// Maximum simultaneous voices per instrument
const MAX_VOICES_PER_INSTRUMENT: usize = 16;

/// A polyphonic voice chain: entire signal chain spawned per note
#[derive(Debug, Clone)]
pub struct VoiceChain {
    pub instrument_id: InstrumentId,
    pub pitch: u8,
    pub group_id: i32,
    pub midi_node_id: i32,
    pub source_node: i32,
    pub spawn_time: Instant,
}

#[derive(Debug, Clone)]
pub struct InstrumentNodes {
    pub source: Option<i32>,
    pub lfo: Option<i32>,
    pub filter: Option<i32>,
    pub effects: Vec<i32>,  // only enabled effects
    pub output: i32,
}

impl InstrumentNodes {
    pub fn all_node_ids(&self) -> Vec<i32> {
        let mut ids = Vec::new();
        if let Some(id) = self.source { ids.push(id); }
        if let Some(id) = self.lfo { ids.push(id); }
        if let Some(id) = self.filter { ids.push(id); }
        ids.extend(&self.effects);
        ids.push(self.output);
        ids
    }
}

pub struct AudioEngine {
    client: Option<OscClient>,
    node_map: HashMap<InstrumentId, InstrumentNodes>,
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
    /// Send synth nodes: (instrument_index, bus_id) -> node_id
    send_node_map: HashMap<(usize, u8), i32>,
    /// Bus output synth nodes: bus_id -> node_id
    bus_node_map: HashMap<u8, i32>,
    /// Active poly voice chains (full signal chain per note)
    voice_chains: Vec<VoiceChain>,
    /// Next available voice bus (audio)
    next_voice_audio_bus: i32,
    /// Next available voice bus (control)
    next_voice_control_bus: i32,
    /// Meter synth node ID
    meter_node_id: Option<i32>,
    /// Sample buffer mapping: BufferId -> SuperCollider buffer number
    buffer_map: HashMap<BufferId, i32>,
    /// Next available buffer number for SuperCollider
    #[allow(dead_code)]
    next_bufnum: i32,
    /// Active disk recording session
    recording: Option<RecordingState>,
    /// Buffer pending free after recording stop (bufnum, when to free)
    pending_buffer_free: Option<(i32, Instant)>,
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
            voice_chains: Vec::new(),
            next_voice_audio_bus: 16,
            next_voice_control_bus: 0,
            meter_node_id: None,
            buffer_map: HashMap::new(),
            next_bufnum: 100, // Start at 100 to avoid conflicts with built-in buffers
            recording: None,
            pending_buffer_free: None,
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

    #[allow(dead_code)]
    pub fn is_compiling(&self) -> bool {
        self.is_compiling
    }

    #[allow(dead_code)]
    pub fn start_server(&mut self) -> Result<(), String> {
        self.start_server_with_devices(None, None)
    }

    pub fn start_server_with_devices(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<(), String> {
        if self.scsynth_process.is_some() {
            return Err("Server already running".to_string());
        }

        self.server_status = ServerStatus::Starting;

        let scsynth_paths = [
            "scsynth",
            "/Applications/SuperCollider.app/Contents/Resources/scsynth",
            "/usr/local/bin/scsynth",
            "/usr/bin/scsynth",
        ];

        // Build args: base port + optional device flags
        let mut args: Vec<String> = vec!["-u".to_string(), "57110".to_string()];

        // Resolve "System Default" to actual device names so we always
        // pass -H to scsynth. Without -H, scsynth probes all devices
        // and can crash on incompatible ones (e.g. iPhone continuity mic).
        let (default_output, default_input) = super::devices::default_device_names();
        let resolved_input = input_device
            .map(|s| s.to_string())
            .or(default_input);
        let resolved_output = output_device
            .map(|s| s.to_string())
            .or(default_output);

        match (resolved_input.as_deref(), resolved_output.as_deref()) {
            (Some(inp), Some(out)) if inp != out => {
                args.push("-H".to_string());
                args.push(inp.to_string());
                args.push(out.to_string());
            }
            (Some(dev), None) | (None, Some(dev)) => {
                args.push("-H".to_string());
                args.push(dev.to_string());
            }
            (Some(dev), Some(_)) => {
                // Same device for both
                args.push("-H".to_string());
                args.push(dev.to_string());
            }
            (None, None) => {}
        }

        // Redirect scsynth output to a log file for crash diagnostics
        let log_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ilex")
            .join("scsynth.log");
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        let stdout_file = fs::File::create(&log_path).ok();
        let stderr_file = stdout_file.as_ref().and_then(|f| f.try_clone().ok());

        let mut child = None;
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        for path in &scsynth_paths {
            match Command::new(path)
                .args(&arg_refs)
                .stdout(stdout_file.as_ref()
                    .and_then(|f| f.try_clone().ok())
                    .map(Stdio::from)
                    .unwrap_or_else(Stdio::null))
                .stderr(stderr_file.as_ref()
                    .and_then(|f| f.try_clone().ok())
                    .map(Stdio::from)
                    .unwrap_or_else(Stdio::null))
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
            Some(mut c) => {
                self.server_status = ServerStatus::Running;
                thread::sleep(Duration::from_millis(500));

                // Verify scsynth didn't crash during startup
                match c.try_wait() {
                    Ok(Some(status)) => {
                        self.server_status = ServerStatus::Error;
                        Err(format!(
                            "scsynth crashed ({}) — see {}",
                            status, log_path.display()
                        ))
                    }
                    _ => {
                        self.scsynth_process = Some(c);
                        Ok(())
                    }
                }
            }
            None => {
                self.server_status = ServerStatus::Error;
                Err("Could not find scsynth. Install SuperCollider.".to_string())
            }
        }
    }

    /// Check if the scsynth child process has exited unexpectedly.
    /// Returns `Some(message)` if it died, `None` if healthy.
    pub fn check_server_health(&mut self) -> Option<String> {
        if let Some(ref mut child) = self.scsynth_process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.scsynth_process = None;
                    self.is_running = false;
                    self.client = None;
                    self.server_status = ServerStatus::Error;
                    self.groups_created = false;
                    Some(format!("scsynth exited ({})", status))
                }
                _ => None,
            }
        } else if self.is_running {
            // is_running but no process — stale state
            self.is_running = false;
            self.server_status = ServerStatus::Error;
            self.groups_created = false;
            Some("scsynth process lost".to_string())
        } else {
            None
        }
    }

    pub fn stop_server(&mut self) {
        self.stop_recording();
        self.disconnect();
        if let Some(mut child) = self.scsynth_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.server_status = ServerStatus::Stopped;
    }

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

    fn run_sclang(scd_path: &PathBuf) -> Result<String, String> {
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
        client.send_message("/notify", vec![rosc::OscType::Int(1)])?;
        self.client = Some(client);
        self.is_running = true;
        self.server_status = ServerStatus::Connected;
        Ok(())
    }

    fn restart_meter(&mut self) {
        if let Some(node_id) = self.meter_node_id.take() {
            if let Some(ref client) = self.client {
                let _ = client.free_node(node_id);
            }
        }
        if let Some(ref client) = self.client {
            let node_id = self.next_node_id;
            self.next_node_id += 1;
            let args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("ilex_meter".to_string()),
                rosc::OscType::Int(node_id),
                rosc::OscType::Int(3), // addAfter
                rosc::OscType::Int(GROUP_OUTPUT),
            ];
            if client.send_message("/s_new", args).is_ok() {
                self.meter_node_id = Some(node_id);
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.stop_recording();
        if let Some(ref client) = self.client {
            if let Some(node_id) = self.meter_node_id.take() {
                let _ = client.free_node(node_id);
            }
            for nodes in self.node_map.values() {
                for node_id in nodes.all_node_ids() {
                    let _ = client.free_node(node_id);
                }
            }
            // Free all loaded sample buffers
            for &bufnum in self.buffer_map.values() {
                let _ = client.free_buffer(bufnum);
            }
        }
        self.node_map.clear();
        self.send_node_map.clear();
        self.bus_node_map.clear();
        self.bus_audio_buses.clear();
        self.voice_chains.clear();
        self.buffer_map.clear();
        self.bus_allocator.reset();
        self.groups_created = false;
        self.client = None;
        self.is_running = false;
        if self.scsynth_process.is_some() {
            self.server_status = ServerStatus::Running;
        } else {
            self.server_status = ServerStatus::Stopped;
        }
    }

    fn ensure_groups(&mut self) -> Result<(), String> {
        if self.groups_created {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;
        client.create_group(GROUP_SOURCES, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_PROCESSING, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_OUTPUT, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_RECORD, 1, 0).map_err(|e| e.to_string())?;
        self.groups_created = true;
        Ok(())
    }

    fn source_synth_def(source: SourceType, registry: &CustomSynthDefRegistry) -> String {
        source.synth_def_name_with_registry(registry)
    }

    fn filter_synth_def(ft: FilterType) -> &'static str {
        ft.synth_def_name()
    }

    fn effect_synth_def(et: EffectType) -> &'static str {
        et.synth_def_name()
    }

    /// Rebuild all routing based on instrument state.
    /// Per instrument, create a deterministic synth chain:
    /// 1. Source synth
    /// 2. Optional filter synth
    /// 3. Effect synths in order
    /// 4. Output synth with level/pan/mute
    pub fn rebuild_instrument_routing(&mut self, state: &InstrumentState, session: &SessionState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }

        self.ensure_groups()?;

        // Free all existing synths and voices
        if let Some(ref client) = self.client {
            for nodes in self.node_map.values() {
                for node_id in nodes.all_node_ids() {
                    let _ = client.free_node(node_id);
                }
            }
            for &node_id in self.send_node_map.values() {
                let _ = client.free_node(node_id);
            }
            for &node_id in self.bus_node_map.values() {
                let _ = client.free_node(node_id);
            }
            for chain in self.voice_chains.drain(..) {
                let _ = client.free_node(chain.group_id);
            }
        }
        self.node_map.clear();
        self.send_node_map.clear();
        self.bus_node_map.clear();
        self.bus_audio_buses.clear();
        self.bus_allocator.reset();

        // Allocate audio buses for each mixer bus first (needed by BusIn instruments)
        for bus in &session.buses {
            let bus_audio = self.bus_allocator.get_or_alloc_audio_bus(
                u32::MAX - bus.id as u32,
                "bus_out",
            );
            self.bus_audio_buses.insert(bus.id, bus_audio);
        }

        // For each instrument, create a linear chain of synths
        // We don't create static source synths for polyphonic instruments (voices are spawned dynamically)
        // But we still need the output synth for summing voice output

        for instrument in &state.instruments {
            let mut source_node: Option<i32> = None;
            let mut lfo_node: Option<i32> = None;
            let mut filter_node: Option<i32> = None;
            let mut effect_nodes: Vec<i32> = Vec::new();

            // Allocate the audio bus that voices/source write to
            let source_out_bus = self.bus_allocator.get_or_alloc_audio_bus(instrument.id, "source_out");
            let mut current_bus = source_out_bus;

            // For AudioIn instruments, create a persistent audio input synth
            if instrument.source.is_audio_input() {
                let node_id = self.next_node_id;
                self.next_node_id += 1;

                let mut params: Vec<(String, f32)> = vec![
                    ("out".to_string(), source_out_bus as f32),
                    ("strip_id".to_string(), instrument.id as f32),
                ];
                // Add source params (gain, channel, test_tone, test_freq)
                for p in &instrument.source_params {
                    let val = match &p.value {
                        crate::state::param::ParamValue::Float(v) => *v,
                        crate::state::param::ParamValue::Int(v) => *v as f32,
                        crate::state::param::ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                    };
                    // Gate gain to 0 when instrument is inactive
                    let val = if p.name == "gain" && !instrument.active {
                        0.0
                    } else {
                        val
                    };
                    params.push((p.name.clone(), val));
                }

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "ilex_audio_in",
                    node_id,
                    GROUP_SOURCES,
                    &params,
                ).map_err(|e| e.to_string())?;

                source_node = Some(node_id);
            } else if instrument.source.is_bus_in() {
                // BusIn instruments read from an internal mixer bus
                let node_id = self.next_node_id;
                self.next_node_id += 1;

                // Look up which bus to read from
                let bus_id = instrument.source_params.iter()
                    .find(|p| p.name == "bus")
                    .map(|p| match &p.value {
                        crate::state::param::ParamValue::Int(v) => *v as u8,
                        _ => 1,
                    })
                    .unwrap_or(1);

                let bus_audio_bus = self.bus_audio_buses.get(&bus_id).copied().unwrap_or(16);

                let gain = instrument.source_params.iter()
                    .find(|p| p.name == "gain")
                    .map(|p| match &p.value {
                        crate::state::param::ParamValue::Float(v) => *v,
                        _ => 1.0,
                    })
                    .unwrap_or(1.0);

                let params: Vec<(String, f32)> = vec![
                    ("out".to_string(), source_out_bus as f32),
                    ("in".to_string(), bus_audio_bus as f32),
                    ("gain".to_string(), gain),
                    ("strip_id".to_string(), instrument.id as f32),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "ilex_bus_in",
                    node_id,
                    GROUP_SOURCES,
                    &params,
                ).map_err(|e| e.to_string())?;

                source_node = Some(node_id);
            }
            // For oscillator instruments, voices are spawned dynamically via spawn_voice()

            // LFO (if enabled)
            let lfo_control_bus: Option<i32> = if instrument.lfo.enabled {
                let lfo_node_id = self.next_node_id;
                self.next_node_id += 1;
                let lfo_out_bus = self.bus_allocator.get_or_alloc_control_bus(instrument.id, "lfo_out");

                let params = vec![
                    ("out".to_string(), lfo_out_bus as f32),
                    ("rate".to_string(), instrument.lfo.rate),
                    ("depth".to_string(), instrument.lfo.depth),
                    ("shape".to_string(), instrument.lfo.shape.index() as f32),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "ilex_lfo",
                    lfo_node_id,
                    GROUP_SOURCES, // LFO in sources group so it runs before processing
                    &params,
                ).map_err(|e| e.to_string())?;

                lfo_node = Some(lfo_node_id);
                Some(lfo_out_bus)
            } else {
                None
            };

            // Filter (if present)
            if let Some(ref filter) = instrument.filter {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let filter_out_bus = self.bus_allocator.get_or_alloc_audio_bus(instrument.id, "filter_out");

                // Determine if LFO should modulate the filter cutoff
                let cutoff_mod_bus = if instrument.lfo.enabled && instrument.lfo.target == crate::state::LfoTarget::FilterCutoff {
                    lfo_control_bus.map(|b| b as f32).unwrap_or(-1.0)
                } else {
                    -1.0 // No modulation
                };

                let params = vec![
                    ("in".to_string(), current_bus as f32),
                    ("out".to_string(), filter_out_bus as f32),
                    ("cutoff".to_string(), filter.cutoff.value),
                    ("resonance".to_string(), filter.resonance.value),
                    ("cutoff_mod_in".to_string(), cutoff_mod_bus),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    Self::filter_synth_def(filter.filter_type),
                    node_id,
                    GROUP_PROCESSING,
                    &params,
                ).map_err(|e| e.to_string())?;

                filter_node = Some(node_id);
                current_bus = filter_out_bus;
            }

            // Effects
            for (i, effect) in instrument.effects.iter().enumerate() {
                if !effect.enabled {
                    continue;
                }
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let effect_out_bus = self.bus_allocator.get_or_alloc_audio_bus(
                    instrument.id,
                    &format!("fx_{}_out", i),
                );

                let mut params: Vec<(String, f32)> = vec![
                    ("in".to_string(), current_bus as f32),
                    ("out".to_string(), effect_out_bus as f32),
                ];
                for p in &effect.params {
                    // For SidechainComp, resolve sc_bus to actual SC audio bus number
                    if effect.effect_type == EffectType::SidechainComp && p.name == "sc_bus" {
                        let bus_id = match &p.value {
                            ParamValue::Int(v) => *v as u8,
                            _ => 0,
                        };
                        let sidechain_in = if bus_id == 0 {
                            0.0 // SynthDef uses self as sidechain
                        } else {
                            self.bus_audio_buses.get(&bus_id).copied().unwrap_or(0) as f32
                        };
                        params.push(("sidechain_in".to_string(), sidechain_in));
                        continue;
                    }
                    let val = match &p.value {
                        ParamValue::Float(v) => *v,
                        ParamValue::Int(v) => *v as f32,
                        ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                    };
                    params.push((p.name.clone(), val));
                }

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    Self::effect_synth_def(effect.effect_type),
                    node_id,
                    GROUP_PROCESSING,
                    &params,
                ).map_err(|e| e.to_string())?;

                effect_nodes.push(node_id);
                current_bus = effect_out_bus;
            }

            // Output synth
            let output_node_id;
            {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let any_solo = state.any_instrument_solo();
                let mute = if any_solo { !instrument.solo } else { instrument.mute || session.master_mute };
                let params = vec![
                    ("in".to_string(), current_bus as f32),
                    ("level".to_string(), instrument.level * session.master_level),
                    ("mute".to_string(), if mute { 1.0 } else { 0.0 }),
                    ("pan".to_string(), instrument.pan),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "ilex_output",
                    node_id,
                    GROUP_OUTPUT,
                    &params,
                ).map_err(|e| e.to_string())?;

                output_node_id = node_id;
            }

            self.node_map.insert(instrument.id, InstrumentNodes {
                source: source_node,
                lfo: lfo_node,
                filter: filter_node,
                effects: effect_nodes,
                output: output_node_id,
            });
        }

        // Store bus allocator state for voice bus allocation
        self.next_voice_audio_bus = self.bus_allocator.next_audio_bus;
        self.next_voice_control_bus = self.bus_allocator.next_control_bus;

        // Create send synths
        for (instrument_idx, instrument) in state.instruments.iter().enumerate() {
            // Get the instrument's source_out bus (where voices sum into)
            let instrument_audio_bus = self.bus_allocator.get_audio_bus(instrument.id, "source_out").unwrap_or(16);

            for send in &instrument.sends {
                if !send.enabled || send.level <= 0.0 {
                    continue;
                }
                if let Some(&bus_audio) = self.bus_audio_buses.get(&send.bus_id) {
                    let node_id = self.next_node_id;
                    self.next_node_id += 1;
                    let params = vec![
                        ("in".to_string(), instrument_audio_bus as f32),
                        ("out".to_string(), bus_audio as f32),
                        ("level".to_string(), send.level),
                    ];
                    if let Some(ref client) = self.client {
                        client
                            .create_synth_in_group("ilex_send", node_id, GROUP_OUTPUT, &params)
                            .map_err(|e| e.to_string())?;
                    }
                    self.send_node_map.insert((instrument_idx, send.bus_id), node_id);
                }
            }
        }

        // Create bus output synths
        for bus in &session.buses {
            if let Some(&bus_audio) = self.bus_audio_buses.get(&bus.id) {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let mute = session.effective_bus_mute(bus);
                let params = vec![
                    ("in".to_string(), bus_audio as f32),
                    ("level".to_string(), bus.level),
                    ("mute".to_string(), if mute { 1.0 } else { 0.0 }),
                    ("pan".to_string(), bus.pan),
                ];
                if let Some(ref client) = self.client {
                    client
                        .create_synth_in_group("ilex_bus_out", node_id, GROUP_OUTPUT, &params)
                        .map_err(|e| e.to_string())?;
                }
                self.bus_node_map.insert(bus.id, node_id);
            }
        }

        // (Re)create meter synth
        self.restart_meter();

        Ok(())
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

    /// Update all instrument output mixer params (level, mute, pan) in real-time without rebuilding the graph
    pub fn update_all_instrument_mixer_params(&self, state: &InstrumentState, session: &SessionState) -> Result<(), String> {
        if !self.is_running { return Ok(()); }
        let client = self.client.as_ref().ok_or("Not connected")?;
        let any_solo = state.any_instrument_solo();
        for instrument in &state.instruments {
            if let Some(nodes) = self.node_map.get(&instrument.id) {
                let mute = instrument.mute || session.master_mute || (any_solo && !instrument.solo);
                client.set_param(nodes.output, "level", instrument.level * session.master_level)
                    .map_err(|e| e.to_string())?;
                client.set_param(nodes.output, "mute", if mute { 1.0 } else { 0.0 })
                    .map_err(|e| e.to_string())?;
                client.set_param(nodes.output, "pan", instrument.pan)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Set a source parameter on an instrument in real-time.
    /// Updates the persistent source node (AudioIn) and all active voice source nodes.
    pub fn set_source_param(&self, instrument_id: InstrumentId, param: &str, value: f32) -> Result<(), String> {
        if !self.is_running { return Ok(()); }
        let client = self.client.as_ref().ok_or("Not connected")?;

        // BusIn "bus" param: translate bus_id (1-8) to SC audio bus number
        if param == "bus" {
            let bus_id = value as u8;
            if let Some(&audio_bus) = self.bus_audio_buses.get(&bus_id) {
                if let Some(nodes) = self.node_map.get(&instrument_id) {
                    if let Some(source_node) = nodes.source {
                        let _ = client.set_param(source_node, "in", audio_bus as f32);
                    }
                }
            }
            return Ok(());
        }

        // Set on persistent source node (AudioIn/BusIn instruments)
        if let Some(nodes) = self.node_map.get(&instrument_id) {
            if let Some(source_node) = nodes.source {
                let _ = client.set_param(source_node, param, value);
            }
        }

        // Set on all active voice source nodes (oscillator/sampler instruments)
        for voice in &self.voice_chains {
            if voice.instrument_id == instrument_id {
                let _ = client.set_param(voice.source_node, param, value);
            }
        }

        Ok(())
    }

    /// Spawn a voice for an instrument
    pub fn spawn_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        state: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        let instrument = state.instrument(instrument_id)
            .ok_or_else(|| format!("No instrument with id {}", instrument_id))?;

        // AudioIn and BusIn instruments don't use voice spawning - they have persistent synths
        if instrument.source.is_audio_input() || instrument.source.is_bus_in() {
            return Ok(());
        }

        // Sampler instruments need special handling
        if instrument.source.is_sample() {
            return self.spawn_sampler_voice(instrument_id, pitch, velocity, offset_secs, state, session);
        }

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Voice-steal: if at limit, free oldest by spawn_time
        let count = self.voice_chains.iter().filter(|v| v.instrument_id == instrument_id).count();
        if count >= MAX_VOICES_PER_INSTRUMENT {
            if let Some(pos) = self.voice_chains.iter()
                .enumerate()
                .filter(|(_, v)| v.instrument_id == instrument_id)
                .min_by_key(|(_, v)| v.spawn_time)
                .map(|(i, _)| i)
            {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(instrument_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_node_id;
        self.next_node_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let tuning = session.tuning_a4 as f64;
        let freq = tuning * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        let mut messages: Vec<rosc::OscMessage> = Vec::new();

        // 1. Create group
        messages.push(rosc::OscMessage {
            addr: "/g_new".to_string(),
            args: vec![
                rosc::OscType::Int(group_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(GROUP_SOURCES),
            ],
        });

        // 2. MIDI control node
        let midi_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("ilex_midi".to_string()),
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

        // 3. Source synth
        let source_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String(Self::source_synth_def(instrument.source, &session.custom_synthdefs)),
                rosc::OscType::Int(source_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];
            // Source params
            for p in &instrument.source_params {
                let val = match &p.value {
                    ParamValue::Float(v) => *v,
                    ParamValue::Int(v) => *v as f32,
                    ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                };
                args.push(rosc::OscType::String(p.name.clone()));
                args.push(rosc::OscType::Float(val));
            }
            // Wire control inputs
            args.push(rosc::OscType::String("freq_in".to_string()));
            args.push(rosc::OscType::Float(voice_freq_bus as f32));
            args.push(rosc::OscType::String("gate_in".to_string()));
            args.push(rosc::OscType::Float(voice_gate_bus as f32));
            // Amp envelope (ADSR)
            args.push(rosc::OscType::String("attack".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.release));
            // Output to source_out_bus
            args.push(rosc::OscType::String("out".to_string()));
            args.push(rosc::OscType::Float(source_out_bus as f32));

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
            instrument_id,
            pitch,
            group_id,
            midi_node_id,
            source_node: source_node_id,
            spawn_time: Instant::now(),
        });

        Ok(())
    }

    /// Spawn a sampler voice (separate method for sampler-specific handling)
    fn spawn_sampler_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        state: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        let instrument = state.instrument(instrument_id)
            .ok_or_else(|| format!("No instrument with id {}", instrument_id))?;

        let sampler_config = instrument.sampler_config.as_ref()
            .ok_or("Sampler instrument has no sampler config")?;

        let buffer_id = sampler_config.buffer_id
            .ok_or("Sampler has no buffer loaded")?;

        let bufnum = self.buffer_map.get(&buffer_id)
            .copied()
            .ok_or("Buffer not loaded in audio engine")?;

        // Get slice for this note (or current selected slice)
        let (slice_start, slice_end) = sampler_config.slice_for_note(pitch)
            .map(|s| (s.start, s.end))
            .unwrap_or((0.0, 1.0));

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Voice-steal: if at limit, free oldest by spawn_time
        let count = self.voice_chains.iter().filter(|v| v.instrument_id == instrument_id).count();
        if count >= MAX_VOICES_PER_INSTRUMENT {
            if let Some(pos) = self.voice_chains.iter()
                .enumerate()
                .filter(|(_, v)| v.instrument_id == instrument_id)
                .min_by_key(|(_, v)| v.spawn_time)
                .map(|(i, _)| i)
            {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(instrument_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_node_id;
        self.next_node_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let tuning = session.tuning_a4 as f64;
        let freq = tuning * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        let mut messages: Vec<rosc::OscMessage> = Vec::new();

        // 1. Create group
        messages.push(rosc::OscMessage {
            addr: "/g_new".to_string(),
            args: vec![
                rosc::OscType::Int(group_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(GROUP_SOURCES),
            ],
        });

        // 2. MIDI control node
        let midi_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("ilex_midi".to_string()),
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

        // 3. Sampler synth
        let sampler_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("ilex_sampler".to_string()),
                rosc::OscType::Int(sampler_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];

            // Get rate and amp from source params
            let rate = instrument.source_params.iter()
                .find(|p| p.name == "rate")
                .map(|p| match &p.value {
                    ParamValue::Float(v) => *v,
                    _ => 1.0,
                })
                .unwrap_or(1.0);

            let amp = instrument.source_params.iter()
                .find(|p| p.name == "amp")
                .map(|p| match &p.value {
                    ParamValue::Float(v) => *v,
                    _ => 0.8,
                })
                .unwrap_or(0.8);

            let loop_mode = sampler_config.loop_mode;

            // Sampler params
            args.push(rosc::OscType::String("bufnum".to_string()));
            args.push(rosc::OscType::Float(bufnum as f32));
            args.push(rosc::OscType::String("sliceStart".to_string()));
            args.push(rosc::OscType::Float(slice_start));
            args.push(rosc::OscType::String("sliceEnd".to_string()));
            args.push(rosc::OscType::Float(slice_end));
            args.push(rosc::OscType::String("rate".to_string()));
            args.push(rosc::OscType::Float(rate));
            args.push(rosc::OscType::String("amp".to_string()));
            args.push(rosc::OscType::Float(amp));
            args.push(rosc::OscType::String("loop".to_string()));
            args.push(rosc::OscType::Float(if loop_mode { 1.0 } else { 0.0 }));

            // Wire control inputs (for pitch tracking if enabled)
            if sampler_config.pitch_tracking {
                args.push(rosc::OscType::String("freq_in".to_string()));
                args.push(rosc::OscType::Float(voice_freq_bus as f32));
            }
            args.push(rosc::OscType::String("gate_in".to_string()));
            args.push(rosc::OscType::Float(voice_gate_bus as f32));
            args.push(rosc::OscType::String("vel_in".to_string()));
            args.push(rosc::OscType::Float(voice_vel_bus as f32));

            // Amp envelope (ADSR)
            args.push(rosc::OscType::String("attack".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.release));

            // Output to source_out_bus
            args.push(rosc::OscType::String("out".to_string()));
            args.push(rosc::OscType::Float(source_out_bus as f32));

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
            instrument_id,
            pitch,
            group_id,
            midi_node_id,
            source_node: sampler_node_id,
            spawn_time: Instant::now(),
        });

        Ok(())
    }

    /// Release a specific voice by instrument and pitch (note-off)
    pub fn release_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        offset_secs: f64,
        state: &InstrumentState,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if let Some(pos) = self
            .voice_chains
            .iter()
            .position(|v| v.instrument_id == instrument_id && v.pitch == pitch)
        {
            let chain = self.voice_chains.remove(pos);
            let time = super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(chain.midi_node_id, &[("gate", 0.0)], time)
                .map_err(|e| e.to_string())?;
            // Schedule group free after envelope release completes (+1s margin)
            let release_time = state.instrument(instrument_id)
                .map(|s| s.amp_envelope.release)
                .unwrap_or(1.0);
            let cleanup_time = super::osc_client::osc_time_from_now(
                offset_secs + release_time as f64 + 1.0
            );
            client
                .send_bundle(
                    vec![rosc::OscMessage {
                        addr: "/n_free".to_string(),
                        args: vec![rosc::OscType::Int(chain.group_id)],
                    }],
                    cleanup_time,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Release all active voices
    pub fn release_all_voices(&mut self) {
        if let Some(ref client) = self.client {
            for chain in self.voice_chains.drain(..) {
                let _ = client.free_node(chain.group_id);
            }
        }
    }

    /// Play a one-shot drum sample routed through an instrument's signal chain
    pub fn play_drum_hit_to_instrument(
        &mut self,
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let bufnum = *self.buffer_map.get(&buffer_id).ok_or("Buffer not loaded")?;
        let out_bus = self
            .bus_allocator
            .get_audio_bus(instrument_id, "source_out")
            .unwrap_or(0);

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        client
            .send_message(
                "/s_new",
                vec![
                    rosc::OscType::String("ilex_sampler_oneshot".to_string()),
                    rosc::OscType::Int(node_id),
                    rosc::OscType::Int(0), // addToHead
                    rosc::OscType::Int(GROUP_SOURCES),
                    rosc::OscType::String("bufnum".to_string()),
                    rosc::OscType::Int(bufnum),
                    rosc::OscType::String("amp".to_string()),
                    rosc::OscType::Float(amp),
                    rosc::OscType::String("sliceStart".to_string()),
                    rosc::OscType::Float(slice_start),
                    rosc::OscType::String("sliceEnd".to_string()),
                    rosc::OscType::Float(slice_end),
                    rosc::OscType::String("out".to_string()),
                    rosc::OscType::Int(out_bus), // Route to instrument's source bus
                ],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Get the current master peak level
    pub fn master_peak(&self) -> f32 {
        self.client
            .as_ref()
            .map(|c| {
                let (l, r) = c.meter_peak();
                l.max(r)
            })
            .unwrap_or(0.0)
    }

    /// Get waveform data for an audio input instrument
    pub fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.client
            .as_ref()
            .map(|c| c.audio_in_waveform(instrument_id))
            .unwrap_or_default()
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

    /// Load a single .scsyndef file into the server
    pub fn load_synthdef_file(&self, path: &Path) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if path.extension().map_or(false, |e| e == "scsyndef") {
            let data = fs::read(path).map_err(|e| e.to_string())?;
            client
                .send_message("/d_recv", vec![rosc::OscType::Blob(data)])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // =========================================================================
    // Buffer Management (for Sampler)
    // =========================================================================

    /// Load a sample file into a SuperCollider buffer
    /// Returns the SC buffer number on success
    #[allow(dead_code)]
    pub fn load_sample(&mut self, buffer_id: BufferId, path: &str) -> Result<i32, String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        // Check if already loaded
        if let Some(&bufnum) = self.buffer_map.get(&buffer_id) {
            return Ok(bufnum);
        }

        let bufnum = self.next_bufnum;
        self.next_bufnum += 1;

        client.load_buffer(bufnum, path).map_err(|e| e.to_string())?;

        self.buffer_map.insert(buffer_id, bufnum);
        Ok(bufnum)
    }

    /// Free a sample buffer from SuperCollider
    #[allow(dead_code)]
    pub fn free_sample(&mut self, buffer_id: BufferId) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if let Some(bufnum) = self.buffer_map.remove(&buffer_id) {
            client.free_buffer(bufnum).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get the SuperCollider buffer number for a loaded buffer
    #[allow(dead_code)]
    pub fn get_sc_bufnum(&self, buffer_id: BufferId) -> Option<i32> {
        self.buffer_map.get(&buffer_id).copied()
    }

    /// Check if a buffer is loaded
    #[allow(dead_code)]
    pub fn is_buffer_loaded(&self, buffer_id: BufferId) -> bool {
        self.buffer_map.contains_key(&buffer_id)
    }

    // =========================================================================
    // Automation
    // =========================================================================

    /// Apply an automation value to a target parameter
    /// This updates the appropriate synth node in real-time
    pub fn apply_automation(&self, target: &AutomationTarget, value: f32, state: &InstrumentState, session: &SessionState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;

        match target {
            AutomationTarget::InstrumentLevel(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    let effective_level = value * session.master_level;
                    client.set_param(nodes.output, "level", effective_level)
                        .map_err(|e| e.to_string())?;
                }
            }
            AutomationTarget::InstrumentPan(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    client.set_param(nodes.output, "pan", value)
                        .map_err(|e| e.to_string())?;
                }
            }
            AutomationTarget::FilterCutoff(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(filter_node) = nodes.filter {
                        client.set_param(filter_node, "cutoff", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::FilterResonance(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(filter_node) = nodes.filter {
                        client.set_param(filter_node, "resonance", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::EffectParam(instrument_id, effect_idx, param_idx) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    let instrument = state.instrument(*instrument_id);
                    if let Some(instrument) = instrument {
                        // Count enabled effects before effect_idx to find the right node
                        let enabled_idx = instrument.effects.iter()
                            .take(*effect_idx)
                            .filter(|e| e.enabled)
                            .count();
                        if let Some(&effect_node) = nodes.effects.get(enabled_idx) {
                            if let Some(effect) = instrument.effects.get(*effect_idx) {
                                if let Some(param) = effect.params.get(*param_idx) {
                                    client.set_param(effect_node, &param.name, value)
                                        .map_err(|e| e.to_string())?;
                                }
                            }
                        }
                    }
                }
            }
            AutomationTarget::SampleRate(instrument_id) => {
                for voice in &self.voice_chains {
                    if voice.instrument_id == *instrument_id {
                        client.set_param(voice.source_node, "rate", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::SampleAmp(instrument_id) => {
                for voice in &self.voice_chains {
                    if voice.instrument_id == *instrument_id {
                        client.set_param(voice.source_node, "amp", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Recording (DiskOut)
    // =========================================================================

    /// Buffer number reserved for disk recording (well above sampler range)
    const RECORD_BUFNUM: i32 = 900;

    /// Start recording audio from the given bus to a WAV file.
    pub fn start_recording(&mut self, bus: i32, path: &Path) -> Result<(), String> {
        if self.recording.is_some() {
            return Err("Already recording".to_string());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;

        let path_str = path.to_string_lossy().to_string();
        let node_id = self.next_node_id;
        self.next_node_id += 1;

        // Send all three commands as a single bundle for atomic execution:
        // 1. Allocate ring buffer  2. Open for disk write  3. Create DiskOut synth
        let messages = vec![
            rosc::OscMessage {
                addr: "/b_alloc".to_string(),
                args: vec![
                    rosc::OscType::Int(Self::RECORD_BUFNUM),
                    rosc::OscType::Int(131072),
                    rosc::OscType::Int(2),
                ],
            },
            rosc::OscMessage {
                addr: "/b_write".to_string(),
                args: vec![
                    rosc::OscType::Int(Self::RECORD_BUFNUM),
                    rosc::OscType::String(path_str),
                    rosc::OscType::String("wav".to_string()),
                    rosc::OscType::String("float".to_string()),
                    rosc::OscType::Int(0),
                    rosc::OscType::Int(0),
                    rosc::OscType::Int(1),
                ],
            },
            rosc::OscMessage {
                addr: "/s_new".to_string(),
                args: vec![
                    rosc::OscType::String("ilex_disk_record".to_string()),
                    rosc::OscType::Int(node_id),
                    rosc::OscType::Int(1), // addToTail
                    rosc::OscType::Int(GROUP_RECORD),
                    rosc::OscType::String("bufnum".to_string()),
                    rosc::OscType::Float(Self::RECORD_BUFNUM as f32),
                    rosc::OscType::String("in".to_string()),
                    rosc::OscType::Float(bus as f32),
                ],
            },
        ];
        client.send_bundle(messages, osc_time_immediate())
            .map_err(|e| e.to_string())?;

        self.recording = Some(RecordingState {
            bufnum: Self::RECORD_BUFNUM,
            node_id,
            path: path.to_path_buf(),
            started_at: Instant::now(),
        });

        Ok(())
    }

    /// Stop the active recording and return the path of the recorded file.
    /// The buffer is not freed immediately — call `poll_pending_buffer_free()` in the
    /// main loop to free it after SuperCollider has flushed the file to disk.
    pub fn stop_recording(&mut self) -> Option<PathBuf> {
        let rec = self.recording.take()?;
        if let Some(ref client) = self.client {
            // Bundle node free + buffer close for atomic execution
            let messages = vec![
                rosc::OscMessage {
                    addr: "/n_free".to_string(),
                    args: vec![rosc::OscType::Int(rec.node_id)],
                },
                rosc::OscMessage {
                    addr: "/b_close".to_string(),
                    args: vec![rosc::OscType::Int(rec.bufnum)],
                },
            ];
            let _ = client.send_bundle(messages, osc_time_immediate());
            // Defer buffer free to give scsynth time to flush the file
            self.pending_buffer_free = Some((rec.bufnum, Instant::now()));
        }
        Some(rec.path)
    }

    /// Free any pending recording buffer after a delay.
    /// Returns true if a buffer was freed this call.
    pub fn poll_pending_buffer_free(&mut self) -> bool {
        if let Some((bufnum, when)) = self.pending_buffer_free {
            if when.elapsed() >= Duration::from_millis(500) {
                if let Some(ref client) = self.client {
                    let _ = client.free_buffer(bufnum);
                }
                self.pending_buffer_free = None;
                return true;
            }
        }
        false
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn recording_elapsed(&self) -> Option<Duration> {
        self.recording.as_ref().map(|r| r.started_at.elapsed())
    }

    pub fn recording_path(&self) -> Option<&Path> {
        self.recording.as_ref().map(|r| r.path.as_path())
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
