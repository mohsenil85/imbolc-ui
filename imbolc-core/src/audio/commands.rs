//! Audio command and feedback types for the audio thread abstraction.
//!
//! Phase 3: AudioHandle serializes commands through an MPSC channel to a
//! dedicated audio thread and consumes feedback updates each frame.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::action::VstTarget;
use crate::audio::snapshot::{AutomationSnapshot, InstrumentSnapshot, PianoRollSnapshot, SessionSnapshot};
use crate::state::automation::AutomationTarget;
use crate::state::vst_plugin::VstPluginId;
use crate::state::{BufferId, EffectId, InstrumentId};

/// Commands sent from the main thread to the audio engine.
///
/// Commands either carry their own data, use reply channels for
/// synchronous operations, or rely on snapshots previously provided via
/// UpdateState / UpdatePianoRollData / UpdateAutomationLanes.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AudioCmd {
    // ── Server lifecycle ──────────────────────────────────────────
    Connect {
        server_addr: String,
        reply: Sender<std::io::Result<()>>,
    },
    Disconnect,
    StartServer {
        input_device: Option<String>,
        output_device: Option<String>,
        reply: Sender<Result<(), String>>,
    },
    StopServer,
    RestartServer {
        input_device: Option<String>,
        output_device: Option<String>,
        server_addr: String,
    },
    CompileSynthDefs {
        scd_path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    LoadSynthDefs {
        dir: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    LoadSynthDefFile {
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },

    // ── State snapshots ───────────────────────────────────────────
    UpdateState {
        instruments: InstrumentSnapshot,
        session: SessionSnapshot,
    },
    UpdatePianoRollData {
        piano_roll: PianoRollSnapshot,
    },
    UpdateAutomationLanes {
        lanes: AutomationSnapshot,
    },

    // ── Playback control ──────────────────────────────────────────
    SetPlaying {
        playing: bool,
    },
    ResetPlayhead,
    SetBpm {
        bpm: f32,
    },

    // ── Routing & mixing ──────────────────────────────────────────
    RebuildRouting,
    RebuildInstrumentRouting {
        instrument_id: InstrumentId,
    },
    UpdateMixerParams,
    SetBusMixerParams {
        bus_id: u8,
        level: f32,
        mute: bool,
        pan: f32,
    },
    SetSourceParam {
        instrument_id: InstrumentId,
        param: String,
        value: f32,
    },
    SetEqParam {
        instrument_id: InstrumentId,
        param: String,
        value: f32,
    },
    /// Targeted /n_set to filter node (no routing rebuild).
    SetFilterParam {
        instrument_id: InstrumentId,
        param: String,
        value: f32,
    },
    /// Targeted /n_set to effect node (no routing rebuild).
    SetEffectParam {
        instrument_id: InstrumentId,
        effect_id: EffectId,
        param: String,
        value: f32,
    },
    /// Targeted /n_set to LFO node (no routing rebuild).
    SetLfoParam {
        instrument_id: InstrumentId,
        param: String,
        value: f32,
    },
    SetInstrumentMixerParams {
        instrument_id: InstrumentId,
        level: f32,
        pan: f32,
        mute: bool,
        solo: bool,
    },
    SetMasterParams {
        level: f32,
        mute: bool,
    },

    // ── Voice management ──────────────────────────────────────────
    SpawnVoice {
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
    },
    ReleaseVoice {
        instrument_id: InstrumentId,
        pitch: u8,
        offset_secs: f64,
    },
    RegisterActiveNote {
        instrument_id: InstrumentId,
        pitch: u8,
        duration_ticks: u32,
    },
    ClearActiveNotes,
    ReleaseAllVoices,
    PlayDrumHit {
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
        rate: f32,
        offset_secs: f64,
    },

    // ── Samples ───────────────────────────────────────────────────
    LoadSample {
        buffer_id: BufferId,
        path: String,
        reply: Sender<Result<i32, String>>,
    },
    FreeSamples {
        buffer_ids: Vec<BufferId>,
    },

    // ── Recording ─────────────────────────────────────────────────
    StartRecording {
        bus: i32,
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    StopRecording {
        reply: Sender<Option<PathBuf>>,
    },
    StartInstrumentRender {
        instrument_id: InstrumentId,
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    StartMasterBounce {
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    StartStemExport {
        stems: Vec<(InstrumentId, PathBuf)>,
        reply: Sender<Result<(), String>>,
    },
    CancelExport,

    // ── Automation ────────────────────────────────────────────────
    ApplyAutomation {
        target: AutomationTarget,
        value: f32,
    },

    // ── VST parameter control ──────────────────────────────────
    QueryVstParams {
        instrument_id: InstrumentId,
        target: VstTarget,
    },
    SetVstParam {
        instrument_id: InstrumentId,
        target: VstTarget,
        param_index: u32,
        value: f32,
    },
    SaveVstState {
        instrument_id: InstrumentId,
        target: VstTarget,
        path: PathBuf,
    },
    LoadVstState {
        instrument_id: InstrumentId,
        target: VstTarget,
        path: PathBuf,
    },

    // ── Lifecycle ─────────────────────────────────────────────────
    Shutdown,
}

/// Feedback sent from the audio thread back to the main thread.
///
/// In Phase 3 these are received via mpsc::Receiver and polled each frame.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AudioFeedback {
    PlayheadPosition(u32),
    BpmUpdate(f32),
    DrumSequencerStep {
        instrument_id: InstrumentId,
        step: usize,
    },
    ServerStatus {
        status: super::ServerStatus,
        message: String,
        server_running: bool,
    },
    RecordingState {
        is_recording: bool,
        elapsed_secs: u64,
    },
    RecordingStopped(PathBuf),
    RenderComplete {
        instrument_id: InstrumentId,
        path: PathBuf,
    },
    CompileResult(Result<String, String>),
    PendingBufferFreed,
    VstParamsDiscovered {
        instrument_id: InstrumentId,
        target: VstTarget,
        vst_plugin_id: VstPluginId,
        params: Vec<(u32, String, Option<String>, f32)>, // (index, name, label, default)
    },
    VstStateSaved {
        instrument_id: InstrumentId,
        target: VstTarget,
        path: PathBuf,
    },
    ExportComplete {
        kind: ExportKind,
        paths: Vec<PathBuf>,
    },
    ExportProgress {
        progress: f32,
    },
    /// The scsynth server process crashed or became unreachable.
    /// All tracked nodes have been invalidated.
    ServerCrashed {
        message: String,
    },
}

/// Export operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    MasterBounce,
    StemExport,
}
