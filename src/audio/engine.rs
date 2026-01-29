use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use super::bus_allocator::BusAllocator;
use super::osc_client::OscClient;
use crate::state::{AutomationTarget, BufferId, CustomSynthDefRegistry, EffectType, FilterType, OscType, Param, ParamValue, StripId, StripState};

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

/// Maximum simultaneous voices per strip
const MAX_VOICES_PER_STRIP: usize = 16;

/// A polyphonic voice chain: entire signal chain spawned per note
#[derive(Debug, Clone)]
pub struct VoiceChain {
    pub strip_id: StripId,
    pub pitch: u8,
    pub group_id: i32,
    pub midi_node_id: i32,
}

pub struct AudioEngine {
    client: Option<OscClient>,
    node_map: HashMap<StripId, Vec<i32>>, // strip_id -> list of node_ids for that strip's static chain
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
    /// Send synth nodes: (strip_index, bus_id) -> node_id
    send_node_map: HashMap<(usize, u8), i32>,
    /// Bus output synth nodes: bus_id -> node_id
    bus_node_map: HashMap<u8, i32>,
    /// Active poly voice chains (full signal chain per note)
    voice_chains: Vec<VoiceChain>,
    /// Next available voice bus (audio)
    next_voice_audio_bus: i32,
    /// Next available voice bus (control)
    next_voice_control_bus: i32,
    /// Next group ID for voice groups
    next_group_id: i32,
    /// Meter synth node ID
    meter_node_id: Option<i32>,
    /// Sample buffer mapping: BufferId -> SuperCollider buffer number
    buffer_map: HashMap<BufferId, i32>,
    /// Next available buffer number for SuperCollider
    next_bufnum: i32,
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
            next_group_id: 1000,
            meter_node_id: None,
            buffer_map: HashMap::new(),
            next_bufnum: 100, // Start at 100 to avoid conflicts with built-in buffers
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

    pub fn start_server(&mut self) -> Result<(), String> {
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
                thread::sleep(Duration::from_millis(500));
                Ok(())
            }
            None => {
                self.server_status = ServerStatus::Error;
                Err("Could not find scsynth. Install SuperCollider.".to_string())
            }
        }
    }

    pub fn stop_server(&mut self) {
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
                rosc::OscType::String("tuidaw_meter".to_string()),
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
        if let Some(ref client) = self.client {
            if let Some(node_id) = self.meter_node_id.take() {
                let _ = client.free_node(node_id);
            }
            for nodes in self.node_map.values() {
                for &node_id in nodes {
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
        self.groups_created = true;
        Ok(())
    }

    fn osc_synth_def(osc: OscType, registry: &CustomSynthDefRegistry) -> String {
        osc.synth_def_name_with_registry(registry)
    }

    fn filter_synth_def(ft: FilterType) -> &'static str {
        ft.synth_def_name()
    }

    fn effect_synth_def(et: EffectType) -> &'static str {
        et.synth_def_name()
    }

    /// Rebuild all routing based on strip state.
    /// Per strip, create a deterministic synth chain:
    /// 1. Source synth (osc)
    /// 2. Optional filter synth
    /// 3. Effect synths in order
    /// 4. Output synth with level/pan/mute
    pub fn rebuild_strip_routing(&mut self, state: &StripState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }

        self.ensure_groups()?;

        // Free all existing synths and voices
        if let Some(ref client) = self.client {
            for nodes in self.node_map.values() {
                for &node_id in nodes {
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

        // For each strip, create a linear chain of synths
        // We don't create static source synths for polyphonic strips (voices are spawned dynamically)
        // But we still need the output synth for summing voice output

        for strip in &state.strips {
            let mut strip_nodes: Vec<i32> = Vec::new();

            // Allocate the audio bus that voices/source write to
            let source_out_bus = self.bus_allocator.get_or_alloc_audio_bus(strip.id, "source_out");
            let mut current_bus = source_out_bus;

            // For AudioIn strips, create a persistent audio input synth
            if strip.source.is_audio_input() {
                let node_id = self.next_node_id;
                self.next_node_id += 1;

                let mut params: Vec<(String, f32)> = vec![
                    ("out".to_string(), source_out_bus as f32),
                    ("strip_id".to_string(), strip.id as f32),
                ];
                // Add source params (gain, channel, test_tone, test_freq)
                for p in &strip.source_params {
                    let val = match &p.value {
                        crate::state::param::ParamValue::Float(v) => *v,
                        crate::state::param::ParamValue::Int(v) => *v as f32,
                        crate::state::param::ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                    };
                    params.push((p.name.clone(), val));
                }

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "tuidaw_audio_in",
                    node_id,
                    GROUP_SOURCES,
                    &params,
                ).map_err(|e| e.to_string())?;

                strip_nodes.push(node_id);
            }
            // For oscillator strips, voices are spawned dynamically via spawn_voice()

            // LFO (if enabled)
            let lfo_control_bus: Option<i32> = if strip.lfo.enabled {
                let lfo_node_id = self.next_node_id;
                self.next_node_id += 1;
                let lfo_out_bus = self.bus_allocator.get_or_alloc_control_bus(strip.id, "lfo_out");

                let params = vec![
                    ("out".to_string(), lfo_out_bus as f32),
                    ("rate".to_string(), strip.lfo.rate),
                    ("depth".to_string(), strip.lfo.depth),
                    ("shape".to_string(), strip.lfo.shape.index() as f32),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "tuidaw_lfo",
                    lfo_node_id,
                    GROUP_SOURCES, // LFO in sources group so it runs before processing
                    &params,
                ).map_err(|e| e.to_string())?;

                strip_nodes.push(lfo_node_id);
                Some(lfo_out_bus)
            } else {
                None
            };

            // Filter (if present)
            if let Some(ref filter) = strip.filter {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let filter_out_bus = self.bus_allocator.get_or_alloc_audio_bus(strip.id, "filter_out");

                // Determine if LFO should modulate the filter cutoff
                let cutoff_mod_bus = if strip.lfo.enabled && strip.lfo.target == crate::state::LfoTarget::FilterCutoff {
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

                strip_nodes.push(node_id);
                current_bus = filter_out_bus;
            }

            // Effects
            for (i, effect) in strip.effects.iter().enumerate() {
                if !effect.enabled {
                    continue;
                }
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let effect_out_bus = self.bus_allocator.get_or_alloc_audio_bus(
                    strip.id,
                    &format!("fx_{}_out", i),
                );

                let mut params: Vec<(String, f32)> = vec![
                    ("in".to_string(), current_bus as f32),
                    ("out".to_string(), effect_out_bus as f32),
                ];
                for p in &effect.params {
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

                strip_nodes.push(node_id);
                current_bus = effect_out_bus;
            }

            // Output synth
            {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let mute = state.effective_strip_mute(strip);
                let params = vec![
                    ("in".to_string(), current_bus as f32),
                    ("level".to_string(), strip.level * state.master_level),
                    ("mute".to_string(), if mute { 1.0 } else { 0.0 }),
                    ("pan".to_string(), strip.pan),
                ];

                let client = self.client.as_ref().ok_or("Not connected")?;
                client.create_synth_in_group(
                    "tuidaw_output",
                    node_id,
                    GROUP_OUTPUT,
                    &params,
                ).map_err(|e| e.to_string())?;

                strip_nodes.push(node_id);
            }

            self.node_map.insert(strip.id, strip_nodes);
        }

        // Store bus allocator state for voice bus allocation
        self.next_voice_audio_bus = self.bus_allocator.next_audio_bus;
        self.next_voice_control_bus = self.bus_allocator.next_control_bus;

        // Sync group IDs with node IDs to avoid collisions in SuperCollider's shared ID space
        self.next_group_id = self.next_node_id;

        // Allocate audio buses for each mixer bus
        let bus_audio_base = 200;
        for bus in &state.buses {
            self.bus_audio_buses.insert(bus.id, bus_audio_base + (bus.id as i32 - 1) * 2);
        }

        // Create send synths
        for (strip_idx, strip) in state.strips.iter().enumerate() {
            // Get the strip's source_out bus (where voices sum into)
            let strip_audio_bus = self.bus_allocator.get_audio_bus(strip.id, "source_out").unwrap_or(16);

            for send in &strip.sends {
                if !send.enabled || send.level <= 0.0 {
                    continue;
                }
                if let Some(&bus_audio) = self.bus_audio_buses.get(&send.bus_id) {
                    let node_id = self.next_node_id;
                    self.next_node_id += 1;
                    let params = vec![
                        ("in".to_string(), strip_audio_bus as f32),
                        ("out".to_string(), bus_audio as f32),
                        ("level".to_string(), send.level),
                    ];
                    if let Some(ref client) = self.client {
                        client
                            .create_synth_in_group("tuidaw_send", node_id, GROUP_OUTPUT, &params)
                            .map_err(|e| e.to_string())?;
                    }
                    self.send_node_map.insert((strip_idx, send.bus_id), node_id);
                }
            }
        }

        // Create bus output synths
        for bus in &state.buses {
            if let Some(&bus_audio) = self.bus_audio_buses.get(&bus.id) {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let mute = state.effective_bus_mute(bus);
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

    /// Spawn a voice for a strip
    pub fn spawn_voice(
        &mut self,
        strip_id: StripId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        _polyphonic: bool,
        state: &StripState,
    ) -> Result<(), String> {
        let strip = state.strip(strip_id)
            .ok_or_else(|| format!("No strip with id {}", strip_id))?;

        // AudioIn strips don't use voice spawning - they have a persistent synth
        if strip.source.is_audio_input() {
            return Ok(());
        }

        // Sampler strips need special handling
        if strip.source.is_sampler() {
            return self.spawn_sampler_voice(strip_id, pitch, velocity, offset_secs, state);
        }

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Voice-steal: if at limit, free oldest
        let count = self.voice_chains.iter().filter(|v| v.strip_id == strip_id).count();
        if count >= MAX_VOICES_PER_STRIP {
            if let Some(pos) = self.voice_chains.iter().position(|v| v.strip_id == strip_id) {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(strip_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_group_id;
        self.next_group_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let freq = 440.0 * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

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
                rosc::OscType::String("tuidaw_midi".to_string()),
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

        // 3. Source oscillator
        let osc_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String(Self::osc_synth_def(strip.source, &state.custom_synthdefs)),
                rosc::OscType::Int(osc_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];
            // Source params
            for p in &strip.source_params {
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
            args.push(rosc::OscType::Float(strip.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.release));
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
            strip_id,
            pitch,
            group_id,
            midi_node_id,
        });

        Ok(())
    }

    /// Spawn a sampler voice (separate method for sampler-specific handling)
    fn spawn_sampler_voice(
        &mut self,
        strip_id: StripId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        state: &StripState,
    ) -> Result<(), String> {
        let strip = state.strip(strip_id)
            .ok_or_else(|| format!("No strip with id {}", strip_id))?;

        let sampler_config = strip.sampler_config.as_ref()
            .ok_or("Sampler strip has no sampler config")?;

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

        // Voice-steal: if at limit, free oldest
        let count = self.voice_chains.iter().filter(|v| v.strip_id == strip_id).count();
        if count >= MAX_VOICES_PER_STRIP {
            if let Some(pos) = self.voice_chains.iter().position(|v| v.strip_id == strip_id) {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(strip_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_group_id;
        self.next_group_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let freq = 440.0 * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

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
                rosc::OscType::String("tuidaw_midi".to_string()),
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
                rosc::OscType::String("tuidaw_sampler".to_string()),
                rosc::OscType::Int(sampler_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];

            // Get rate and amp from source params
            let rate = strip.source_params.iter()
                .find(|p| p.name == "rate")
                .map(|p| match &p.value {
                    ParamValue::Float(v) => *v,
                    _ => 1.0,
                })
                .unwrap_or(1.0);

            let amp = strip.source_params.iter()
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
            args.push(rosc::OscType::Float(strip.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(strip.amp_envelope.release));

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
            strip_id,
            pitch,
            group_id,
            midi_node_id,
        });

        Ok(())
    }

    /// Release a specific voice by strip and pitch (note-off)
    pub fn release_voice(
        &mut self,
        strip_id: StripId,
        pitch: u8,
        offset_secs: f64,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if let Some(pos) = self
            .voice_chains
            .iter()
            .position(|v| v.strip_id == strip_id && v.pitch == pitch)
        {
            let chain = self.voice_chains.remove(pos);
            let time = super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(chain.midi_node_id, &[("gate", 0.0)], time)
                .map_err(|e| e.to_string())?;
            // Schedule group free after 5 seconds (release envelope)
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

    /// Get waveform data for an audio input strip
    pub fn audio_in_waveform(&self, strip_id: u32) -> Vec<f32> {
        self.client
            .as_ref()
            .map(|c| c.audio_in_waveform(strip_id))
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
    pub fn free_sample(&mut self, buffer_id: BufferId) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if let Some(bufnum) = self.buffer_map.remove(&buffer_id) {
            client.free_buffer(bufnum).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get the SuperCollider buffer number for a loaded buffer
    pub fn get_sc_bufnum(&self, buffer_id: BufferId) -> Option<i32> {
        self.buffer_map.get(&buffer_id).copied()
    }

    /// Check if a buffer is loaded
    pub fn is_buffer_loaded(&self, buffer_id: BufferId) -> bool {
        self.buffer_map.contains_key(&buffer_id)
    }

    // =========================================================================
    // Automation
    // =========================================================================

    /// Apply an automation value to a target parameter
    /// This updates the appropriate synth node in real-time
    pub fn apply_automation(&self, target: &AutomationTarget, value: f32, state: &StripState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;

        match target {
            AutomationTarget::StripLevel(strip_id) => {
                // Find the output node for this strip and update level
                if let Some(nodes) = self.node_map.get(strip_id) {
                    if let Some(&output_node) = nodes.last() {
                        let effective_level = value * state.master_level;
                        client.set_param(output_node, "level", effective_level)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::StripPan(strip_id) => {
                if let Some(nodes) = self.node_map.get(strip_id) {
                    if let Some(&output_node) = nodes.last() {
                        client.set_param(output_node, "pan", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::FilterCutoff(strip_id) => {
                // Find filter node (second node in chain if it exists)
                if let Some(nodes) = self.node_map.get(strip_id) {
                    // Filter is typically the first processing node
                    // Skip source nodes (if AudioIn) and look for filter
                    let strip = state.strip(*strip_id);
                    if let Some(strip) = strip {
                        if strip.filter.is_some() {
                            // Filter node follows source (which might be AudioIn)
                            let filter_idx = if strip.source.is_audio_input() { 1 } else { 0 };
                            if let Some(&filter_node) = nodes.get(filter_idx) {
                                client.set_param(filter_node, "cutoff", value)
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }
            }
            AutomationTarget::FilterResonance(strip_id) => {
                if let Some(nodes) = self.node_map.get(strip_id) {
                    let strip = state.strip(*strip_id);
                    if let Some(strip) = strip {
                        if strip.filter.is_some() {
                            let filter_idx = if strip.source.is_audio_input() { 1 } else { 0 };
                            if let Some(&filter_node) = nodes.get(filter_idx) {
                                client.set_param(filter_node, "res", value)
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }
            }
            AutomationTarget::EffectParam(strip_id, effect_idx, param_idx) => {
                if let Some(nodes) = self.node_map.get(strip_id) {
                    let strip = state.strip(*strip_id);
                    if let Some(strip) = strip {
                        // Calculate the effect node index
                        let mut node_idx = if strip.source.is_audio_input() { 1 } else { 0 };
                        if strip.filter.is_some() {
                            node_idx += 1;
                        }
                        node_idx += *effect_idx;

                        if let Some(&effect_node) = nodes.get(node_idx) {
                            if let Some(effect) = strip.effects.get(*effect_idx) {
                                if let Some(param) = effect.params.get(*param_idx) {
                                    client.set_param(effect_node, &param.name, value)
                                        .map_err(|e| e.to_string())?;
                                }
                            }
                        }
                    }
                }
            }
            AutomationTarget::SamplerRate(strip_id) => {
                // For sampler rate, we need to set it on all active voices for this strip
                for voice in &self.voice_chains {
                    if voice.strip_id == *strip_id {
                        // The sampler synth is the second node after MIDI control
                        // Set rate on the voice group's sampler synth
                        // Note: This is a simplification; ideally we'd track sampler node IDs
                        // For now, we update via the MIDI node which won't work directly
                        // A proper implementation would need voice-level tracking
                    }
                }
                // Alternative: Update through source params (will affect next spawned voice)
            }
            AutomationTarget::SamplerAmp(strip_id) => {
                // Similar to SamplerRate - affects voices
                for voice in &self.voice_chains {
                    if voice.strip_id == *strip_id {
                        // Would need proper voice tracking
                    }
                }
            }
        }

        Ok(())
    }

    // Keep old method signature as alias for backward compat during transition
    pub fn rebuild_routing(&mut self, _state: &StripState) -> Result<(), String> {
        self.rebuild_strip_routing(_state)
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
