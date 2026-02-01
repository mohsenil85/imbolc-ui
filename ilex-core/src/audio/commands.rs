//! Audio command and feedback types for the audio thread abstraction.
//!
//! Phase 3: AudioHandle serializes commands through an MPSC channel to a
//! dedicated audio thread and consumes feedback updates each frame.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use crate::state::automation::{AutomationLane, AutomationTarget};
use crate::state::piano_roll::PianoRollState;
use crate::state::{BufferId, InstrumentId, InstrumentState, SessionState};

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
        instruments: InstrumentState,
        session: SessionState,
    },
    UpdatePianoRollData {
        piano_roll: PianoRollState,
    },
    UpdateAutomationLanes {
        lanes: Vec<AutomationLane>,
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
    },

    // ── Samples ───────────────────────────────────────────────────
    LoadSample {
        buffer_id: BufferId,
        path: String,
        reply: Sender<Result<i32, String>>,
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

    // ── Automation ────────────────────────────────────────────────
    ApplyAutomation {
        target: AutomationTarget,
        value: f32,
    },

    // ── Lifecycle ─────────────────────────────────────────────────
    Shutdown,
}

/// Feedback sent from the audio thread back to the main thread.
///
/// In Phase 3 these are received via mpsc::Receiver and polled each frame.
#[derive(Debug)]
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
    CompileResult(Result<String, String>),
    PendingBufferFreed,
}
