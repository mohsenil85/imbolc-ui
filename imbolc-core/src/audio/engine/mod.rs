mod automation;
mod recording;
mod routing;
mod samples;
mod server;
mod voices;
mod vst;

use std::collections::HashMap;
use std::process::Child;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use super::bus_allocator::BusAllocator;
use super::osc_client::OscClientLike;
use crate::state::{BufferId, InstrumentId};

use recording::{ExportRecordingState, RecordingState};

#[allow(dead_code)]
pub type ModuleId = u32;

// SuperCollider group IDs for execution ordering
pub const GROUP_SOURCES: i32 = 100;
pub const GROUP_PROCESSING: i32 = 200;
pub const GROUP_OUTPUT: i32 = 300;
pub const GROUP_RECORD: i32 = 400;

// Wavetable buffer range for VOsc (imbolc_wavetable SynthDef)
pub const WAVETABLE_BUFNUM_START: i32 = 100;
pub const WAVETABLE_NUM_TABLES: i32 = 8;

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

/// VSTPlugin UGen index within wrapper SynthDefs (imbolc_vst_instrument, imbolc_vst_effect).
/// This is 0 because VSTPlugin is the first (and only) UGen in our wrappers.
const VST_UGEN_INDEX: i32 = 0;

/// A polyphonic voice chain: entire signal chain spawned per note
#[derive(Debug, Clone)]
pub struct VoiceChain {
    pub instrument_id: InstrumentId,
    pub pitch: u8,
    pub velocity: f32,
    pub group_id: i32,
    pub midi_node_id: i32,
    pub source_node: i32,
    pub spawn_time: Instant,
    /// If set, voice has been released: (released_at, release_duration_secs)
    pub release_state: Option<(Instant, f32)>,
}

#[derive(Debug, Clone)]
pub struct InstrumentNodes {
    pub source: Option<i32>,
    pub lfo: Option<i32>,
    pub filter: Option<i32>,
    pub eq: Option<i32>,
    pub effects: Vec<i32>,  // only enabled effects
    pub output: i32,
}

impl InstrumentNodes {
    pub fn all_node_ids(&self) -> Vec<i32> {
        let mut ids = Vec::new();
        if let Some(id) = self.source { ids.push(id); }
        if let Some(id) = self.lfo { ids.push(id); }
        if let Some(id) = self.filter { ids.push(id); }
        if let Some(id) = self.eq { ids.push(id); }
        ids.extend(&self.effects);
        ids.push(self.output);
        ids
    }
}

pub struct AudioEngine {
    client: Option<Box<dyn OscClientLike>>,
    pub(crate) node_map: HashMap<InstrumentId, InstrumentNodes>,
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
    /// Instrument final buses: instrument_id -> SC audio bus index (post-effects, pre-mixer)
    pub(crate) instrument_final_buses: HashMap<InstrumentId, i32>,
    /// Active poly voice chains (full signal chain per note)
    voice_chains: Vec<VoiceChain>,
    /// Next available voice bus (audio)
    next_voice_audio_bus: i32,
    /// Next available voice bus (control)
    next_voice_control_bus: i32,
    /// Meter synth node ID
    meter_node_id: Option<i32>,
    /// Analysis synth node IDs (spectrum, LUFS, scope)
    analysis_node_ids: Vec<i32>,
    /// Sample buffer mapping: BufferId -> SuperCollider buffer number
    buffer_map: HashMap<BufferId, i32>,
    /// Next available buffer number for SuperCollider
    #[allow(dead_code)]
    next_bufnum: i32,
    /// Whether wavetable buffers (100–107) have been initialized
    wavetables_initialized: bool,
    /// Active disk recording session
    recording: Option<RecordingState>,
    /// Buffer pending free after recording stop (bufnum, when to free)
    pending_buffer_free: Option<(i32, Instant)>,
    /// Active export session (master bounce or stem export)
    export_state: Option<ExportRecordingState>,
    /// Buffers pending free after export stop
    pending_export_buffer_frees: Vec<(i32, Instant)>,
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
            instrument_final_buses: HashMap::new(),
            voice_chains: Vec::new(),
            next_voice_audio_bus: 16,
            next_voice_control_bus: 0,
            meter_node_id: None,
            analysis_node_ids: Vec::new(),
            buffer_map: HashMap::new(),
            next_bufnum: WAVETABLE_BUFNUM_START + WAVETABLE_NUM_TABLES, // Start after wavetable range
            wavetables_initialized: false,
            recording: None,
            pending_buffer_free: None,
            export_state: None,
            pending_export_buffer_frees: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::osc_client::NullOscClient;
    use crate::state::{AppState, AutomationTarget, FilterConfig, ParamValue};
    use crate::state::instrument::{EffectSlot, EffectType, FilterType, SourceType};

    fn connect_engine() -> AudioEngine {
        let mut engine = AudioEngine::new();
        engine.client = Some(Box::new(NullOscClient::new()));
        engine.is_running = true;
        engine.server_status = ServerStatus::Connected;
        engine
    }

    #[test]
    fn rebuild_routing_creates_nodes_for_audio_in_with_effects_and_sends() {
        let mut engine = connect_engine();
        let mut state = AppState::new();

        let inst_id = state.add_instrument(SourceType::AudioIn);
        if let Some(inst) = state.instruments.instrument_mut(inst_id) {
            inst.filter = Some(FilterConfig::new(FilterType::Lpf));
            inst.lfo.enabled = true;
            inst.effects.push(EffectSlot::new(EffectType::Delay));
            inst.sends[0].enabled = true;
            inst.sends[0].level = 0.5;
        }

        engine
            .rebuild_instrument_routing(&state.instruments, &state.session)
            .expect("rebuild routing");

        let nodes = engine.node_map.get(&inst_id).expect("nodes");
        assert!(nodes.source.is_some());
        assert!(nodes.filter.is_some());
        assert!(nodes.lfo.is_some());
        assert_eq!(nodes.effects.len(), 1);
        assert!(engine.send_node_map.contains_key(&(0, 1)));
        assert_eq!(engine.bus_node_map.len(), state.session.buses.len());
    }

    #[test]
    fn rebuild_routing_handles_bus_in_with_sidechain_effect() {
        let mut engine = connect_engine();
        let mut state = AppState::new();

        let inst_id = state.add_instrument(SourceType::BusIn);
        if let Some(inst) = state.instruments.instrument_mut(inst_id) {
            let mut effect = EffectSlot::new(EffectType::SidechainComp);
            if let Some(param) = effect.params.iter_mut().find(|p| p.name == "sc_bus") {
                param.value = ParamValue::Int(1);
            }
            inst.effects.push(effect);
        }

        engine
            .rebuild_instrument_routing(&state.instruments, &state.session)
            .expect("rebuild routing");

        let nodes = engine.node_map.get(&inst_id).expect("nodes");
        assert!(nodes.source.is_some());
        assert_eq!(nodes.effects.len(), 1);
    }

    #[test]
    fn apply_automation_covers_all_targets() {
        let mut engine = connect_engine();
        let mut state = AppState::new();

        let inst_id = state.add_instrument(SourceType::Saw);
        if let Some(inst) = state.instruments.instrument_mut(inst_id) {
            inst.filter = Some(FilterConfig::new(FilterType::Hpf));
            let mut disabled = EffectSlot::new(EffectType::Delay);
            disabled.enabled = false;
            inst.effects.push(disabled);
            inst.effects.push(EffectSlot::new(EffectType::Reverb));
        }

        engine
            .rebuild_instrument_routing(&state.instruments, &state.session)
            .expect("rebuild routing");

        engine.voice_chains.push(VoiceChain {
            instrument_id: inst_id,
            pitch: 60,
            velocity: 0.8,
            group_id: 0,
            midi_node_id: 0,
            source_node: 1234,
            spawn_time: Instant::now(),
            release_state: None,
        });

        engine
            .apply_automation(
                &AutomationTarget::InstrumentLevel(inst_id),
                0.5,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::InstrumentPan(inst_id),
                -0.25,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::FilterCutoff(inst_id),
                800.0,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::FilterResonance(inst_id),
                0.5,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::EffectParam(inst_id, 1, 0),
                0.7,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::SampleRate(inst_id),
                1.2,
                &state.instruments,
                &state.session,
            )
            .unwrap();
        engine
            .apply_automation(
                &AutomationTarget::SampleAmp(inst_id),
                0.8,
                &state.instruments,
                &state.session,
            )
            .unwrap();
    }

    #[test]
    fn set_source_param_bus_translates_bus_id() {
        let mut engine = connect_engine();
        let mut state = AppState::new();

        let inst_id = state.add_instrument(SourceType::BusIn);
        engine
            .rebuild_instrument_routing(&state.instruments, &state.session)
            .expect("rebuild routing");

        engine
            .set_source_param(inst_id, "bus", 1.0)
            .expect("set_source_param");
    }

    #[test]
    fn set_bus_mixer_params_uses_bus_nodes() {
        let mut engine = connect_engine();
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);

        engine
            .rebuild_instrument_routing(&state.instruments, &state.session)
            .expect("rebuild routing");

        engine
            .set_bus_mixer_params(1, 0.5, false, 0.0)
            .expect("set_bus_mixer_params");
    }

    fn make_voice(inst_id: InstrumentId, pitch: u8, velocity: f32, age_ms: u64) -> VoiceChain {
        VoiceChain {
            instrument_id: inst_id,
            pitch,
            velocity,
            group_id: 0,
            midi_node_id: 0,
            source_node: 0,
            spawn_time: Instant::now() - std::time::Duration::from_millis(age_ms),
            release_state: None,
        }
    }

    fn make_released_voice(
        inst_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        released_ago_ms: u64,
        release_dur: f32,
    ) -> VoiceChain {
        VoiceChain {
            instrument_id: inst_id,
            pitch,
            velocity,
            group_id: 0,
            midi_node_id: 0,
            source_node: 0,
            spawn_time: Instant::now() - std::time::Duration::from_millis(released_ago_ms + 100),
            release_state: Some((
                Instant::now() - std::time::Duration::from_millis(released_ago_ms),
                release_dur,
            )),
        }
    }

    #[test]
    fn test_same_pitch_retrigger() {
        let mut engine = connect_engine();
        let inst_id = 1;

        // Add a voice at pitch 60
        engine.voice_chains.push(make_voice(inst_id, 60, 0.8, 100));

        // Steal for a new note at the same pitch
        engine
            .steal_voice_if_needed(inst_id, 60, 0.9)
            .expect("steal");

        // The old voice should be removed
        assert!(
            engine.voice_chains.iter().all(|v| !(v.instrument_id == inst_id && v.pitch == 60)),
            "same-pitch voice should have been stolen"
        );
    }

    #[test]
    fn test_released_voices_stolen_first() {
        let mut engine = connect_engine();
        let inst_id = 1;

        // Fill to limit with active voices
        for i in 0..MAX_VOICES_PER_INSTRUMENT {
            engine.voice_chains.push(make_voice(inst_id, 40 + i as u8, 0.8, 100));
        }
        // Add an extra released voice (active count is already at limit)
        engine.voice_chains.push(make_released_voice(inst_id, 80, 0.8, 500, 1.0));

        // Trigger steal
        engine
            .steal_voice_if_needed(inst_id, 90, 0.8)
            .expect("steal");

        // The released voice should be gone, not any active voice
        assert!(
            !engine.voice_chains.iter().any(|v| v.pitch == 80 && v.instrument_id == inst_id),
            "released voice should be stolen before active voices"
        );
        // All original active voices should still be present
        assert_eq!(
            engine.voice_chains.iter().filter(|v| v.instrument_id == inst_id).count(),
            MAX_VOICES_PER_INSTRUMENT,
        );
    }

    #[test]
    fn test_lowest_velocity_stolen() {
        let mut engine = connect_engine();
        let inst_id = 1;

        // Fill to limit — all same age, varying velocity
        for i in 0..MAX_VOICES_PER_INSTRUMENT {
            let vel = 0.2 + (i as f32 * 0.05); // 0.2, 0.25, 0.30, ...
            engine.voice_chains.push(make_voice(inst_id, 40 + i as u8, vel, 100));
        }
        let quietest_pitch = engine.voice_chains[0].pitch; // velocity 0.2

        engine
            .steal_voice_if_needed(inst_id, 90, 0.8)
            .expect("steal");

        assert!(
            !engine.voice_chains.iter().any(|v| v.pitch == quietest_pitch && v.instrument_id == inst_id),
            "lowest velocity voice should be stolen"
        );
    }

    #[test]
    fn test_age_tiebreaker() {
        let mut engine = connect_engine();
        let inst_id = 1;

        // Fill to limit — all same velocity, varying age
        for i in 0..MAX_VOICES_PER_INSTRUMENT {
            let age = 1000 - (i as u64 * 50); // oldest first: 1000, 950, 900, ...
            engine.voice_chains.push(make_voice(inst_id, 40 + i as u8, 0.5, age));
        }
        let oldest_pitch = engine.voice_chains[0].pitch; // age 1000ms

        engine
            .steal_voice_if_needed(inst_id, 90, 0.5)
            .expect("steal");

        assert!(
            !engine.voice_chains.iter().any(|v| v.pitch == oldest_pitch && v.instrument_id == inst_id),
            "oldest voice should be stolen as tiebreaker"
        );
    }

    #[test]
    fn test_cleanup_expired_voices() {
        let mut engine = connect_engine();
        let inst_id = 1;

        // Add a voice released long ago (should be cleaned up)
        engine.voice_chains.push(make_released_voice(inst_id, 60, 0.5, 5000, 0.5));
        // Add a voice released recently (should be kept)
        engine.voice_chains.push(make_released_voice(inst_id, 72, 0.5, 100, 1.0));
        // Add an active voice (should be kept)
        engine.voice_chains.push(make_voice(inst_id, 48, 0.8, 200));

        engine.cleanup_expired_voices();

        assert_eq!(engine.voice_chains.len(), 2);
        assert!(engine.voice_chains.iter().any(|v| v.pitch == 72));
        assert!(engine.voice_chains.iter().any(|v| v.pitch == 48));
        assert!(!engine.voice_chains.iter().any(|v| v.pitch == 60));
    }
}
