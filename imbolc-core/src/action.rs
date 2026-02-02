use std::path::PathBuf;

use crate::audio::ServerStatus;
use crate::state::arrangement::{ClipId, PlacementId};
use crate::state::{EqConfig, EffectType, EffectSlot, EnvConfig, FilterConfig, FilterType, InstrumentId, MixerSelection, MusicalSettings, Param, SourceType, VstPluginKind};
use crate::state::ClipboardNote;
use crate::state::drum_sequencer::DrumStep;
use crate::state::automation::{AutomationLaneId, AutomationTarget, CurveType};
use crate::state::custom_synthdef::CustomSynthDef;
use crate::state::instrument_state::InstrumentState;
use crate::state::session::SessionState;

#[derive(Debug)]
pub enum IoFeedback {
    SaveComplete { id: u64, result: Result<String, String> },
    LoadComplete { id: u64, result: Result<(SessionState, InstrumentState, String), String> },
    ImportSynthDefComplete { id: u64, result: Result<(CustomSynthDef, String, PathBuf), String> },
    ImportSynthDefLoaded { id: u64, result: Result<String, String> },
}

/// Data carried by InstrumentAction::Update to apply edits without dispatch reading pane state
#[derive(Debug, Clone)]
pub struct InstrumentUpdate {
    pub id: InstrumentId,
    pub source: SourceType,
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
    pub eq: Option<EqConfig>,
    pub effects: Vec<EffectSlot>,
    pub amp_envelope: EnvConfig,
    pub polyphonic: bool,
    pub active: bool,
}

/// Drum sequencer actions
#[derive(Debug, Clone, PartialEq)]
pub enum SequencerAction {
    ToggleStep(usize, usize),         // (pad_idx, step_idx)
    AdjustVelocity(usize, usize, i8), // (pad_idx, step_idx, delta)
    PlayStop,
    LoadSample(usize),              // pad_idx
    ClearPad(usize),                // pad_idx
    ClearPattern,
    CyclePatternLength,
    NextPattern,
    PrevPattern,
    AdjustPadLevel(usize, f32),     // (pad_idx, delta)
    LoadSampleResult(usize, PathBuf), // (pad_idx, path) — from file browser
    AdjustSwing(f32),               // delta for swing amount
    ApplyEuclidean { pad: usize, pulses: usize, steps: usize, rotation: usize },
    AdjustProbability(usize, usize, f32), // (pad_idx, step_idx, delta)
    ToggleChain,
    AddChainStep(usize),            // pattern_index
    RemoveChainStep(usize),         // position in chain
    MoveChainStep(usize, usize),    // from_position, to_position
    ToggleReverse(usize),              // pad_idx
    AdjustPadPitch(usize, i8),         // (pad_idx, delta semitones)
    AdjustStepPitch(usize, usize, i8), // (pad_idx, step_idx, delta)
    /// Delete steps in region (used by Cut)
    DeleteStepsInRegion {
        start_pad: usize,
        end_pad: usize,
        start_step: usize,
        end_step: usize,
    },
    /// Paste drum steps at cursor
    PasteSteps {
        anchor_pad: usize,
        anchor_step: usize,
        steps: Vec<(usize, usize, DrumStep)>,
    },
}

/// Navigation actions (pane switching, modal stack)
#[derive(Debug, Clone, PartialEq)]
pub enum NavAction {
    SwitchPane(&'static str),
    PushPane(&'static str),
    PopPane,
}

/// Instrument actions
#[derive(Debug, Clone)]
pub enum InstrumentAction {
    Add(SourceType),
    Delete(InstrumentId),
    Edit(InstrumentId),
    Update(Box<InstrumentUpdate>),
    #[allow(dead_code)]
    SetParam(InstrumentId, String, f32),
    AddEffect(InstrumentId, EffectType),
    #[allow(dead_code)]
    RemoveEffect(InstrumentId, usize),
    #[allow(dead_code)]
    MoveEffect(InstrumentId, usize, i8),
    #[allow(dead_code)]
    SetFilter(InstrumentId, Option<FilterType>),
    PlayNote(u8, u8),
    PlayNotes(Vec<u8>, u8),
    Select(usize),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    PlayDrumPad(usize),
    LoadSampleResult(InstrumentId, PathBuf),
    ToggleArp(InstrumentId),
    CycleArpDirection(InstrumentId),
    CycleArpRate(InstrumentId),
    AdjustArpOctaves(InstrumentId, i8),
    AdjustArpGate(InstrumentId, f32),
    CycleChordShape(InstrumentId),
    ClearChordShape(InstrumentId),
    LoadIRResult(InstrumentId, usize, PathBuf), // instrument_id, effect_index, path
    OpenVstEffectParams(InstrumentId, usize), // instrument_id, effect_index
    SetEqParam(InstrumentId, usize, String, f32), // instrument_id, band_index, param_name, value
    ToggleEq(InstrumentId),
}

/// Mixer actions
#[derive(Debug, Clone, PartialEq)]
pub enum MixerAction {
    Move(i8),
    Jump(i8),
    SelectAt(MixerSelection),
    AdjustLevel(f32),
    ToggleMute,
    ToggleSolo,
    CycleSection,
    CycleOutput,
    CycleOutputReverse,
    AdjustSend(u8, f32),
    ToggleSend(u8),
}

/// Piano roll actions — all variants carry the data they need
#[derive(Debug, Clone, PartialEq)]
pub enum PianoRollAction {
    ToggleNote { pitch: u8, tick: u32, duration: u32, velocity: u8, track: usize },
    #[allow(dead_code)]
    MoveCursor(i8, i32),
    PlayStop,
    ToggleLoop,
    SetLoopStart(u32),
    SetLoopEnd(u32),
    #[allow(dead_code)]
    SetBpm(f32),
    #[allow(dead_code)]
    Zoom(i8),
    #[allow(dead_code)]
    ScrollOctave(i8),
    CycleTimeSig,
    TogglePolyMode(usize),
    PlayNote { pitch: u8, velocity: u8, instrument_id: InstrumentId, track: usize },
    PlayNotes { pitches: Vec<u8>, velocity: u8, instrument_id: InstrumentId, track: usize },
    PlayStopRecord,
    AdjustSwing(f32),               // delta for swing amount
    RenderToWav(InstrumentId),
    /// Delete all notes in the given region (used by Cut)
    DeleteNotesInRegion {
        track: usize,
        start_tick: u32,
        end_tick: u32,
        start_pitch: u8,
        end_pitch: u8,
    },
    /// Paste notes at a position from clipboard
    PasteNotes {
        track: usize,
        anchor_tick: u32,
        anchor_pitch: u8,
        notes: Vec<ClipboardNote>,
    },
    BounceToWav,
    ExportStems,
    CancelExport,
}

/// Arrangement/timeline actions
#[derive(Debug, Clone, PartialEq)]
pub enum ArrangementAction {
    TogglePlayMode,
    CreateClip { instrument_id: InstrumentId, length_ticks: u32 },
    CaptureClipFromPianoRoll { instrument_id: InstrumentId },
    DeleteClip(ClipId),
    RenameClip(ClipId, String),
    PlaceClip { clip_id: ClipId, instrument_id: InstrumentId, start_tick: u32 },
    RemovePlacement(PlacementId),
    MovePlacement { placement_id: PlacementId, new_start_tick: u32 },
    ResizePlacement { placement_id: PlacementId, new_length: Option<u32> },
    DuplicatePlacement(PlacementId),
    SelectPlacement(Option<usize>),
    SelectLane(usize),
    MoveCursor(i32),
    ScrollView(i32),
    ZoomIn,
    ZoomOut,
    EnterClipEdit(ClipId),
    ExitClipEdit,
    PlayStop,
}

/// Sample chopper actions
#[derive(Debug, Clone, PartialEq)]
pub enum ChopperAction {
    LoadSample,
    LoadSampleResult(PathBuf),
    AddSlice(f32),           // cursor_pos
    RemoveSlice,
    AssignToPad(usize),
    AutoSlice(usize),
    PreviewSlice,
    SelectSlice(i8),         // +1/-1
    NudgeSliceStart(f32),
    NudgeSliceEnd(f32),
    MoveCursor(i8),          // direction
    CommitAll,               // assign all slices to pads and return
}

/// Audio server actions — Start/Restart carry device selections
#[derive(Debug, Clone, PartialEq)]
pub enum ServerAction {
    Connect,
    Disconnect,
    Start { input_device: Option<String>, output_device: Option<String> },
    Stop,
    CompileSynthDefs,
    LoadSynthDefs,
    Restart { input_device: Option<String>, output_device: Option<String> },
    RecordMaster,
    RecordInput,
}

/// Identifies whether a VST operation targets the instrument source or an effect slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstTarget {
    Source,
    Effect(usize),  // index into instrument.effects[]
}

/// VST parameter actions
#[derive(Debug, Clone, PartialEq)]
pub enum VstParamAction {
    SetParam(InstrumentId, VstTarget, u32, f32),       // instrument_id, target, param_index, value
    AdjustParam(InstrumentId, VstTarget, u32, f32),    // instrument_id, target, param_index, delta
    ResetParam(InstrumentId, VstTarget, u32),           // instrument_id, target, param_index
    DiscoverParams(InstrumentId, VstTarget),
    SaveState(InstrumentId, VstTarget),
}

/// Automation actions
#[derive(Debug, Clone, PartialEq)]
pub enum AutomationAction {
    AddLane(AutomationTarget),
    RemoveLane(AutomationLaneId),
    ToggleLaneEnabled(AutomationLaneId),
    AddPoint(AutomationLaneId, u32, f32),          // lane, tick, value
    RemovePoint(AutomationLaneId, u32),             // lane, tick
    MovePoint(AutomationLaneId, u32, u32, f32),     // lane, old_tick, new_tick, new_value
    SetCurveType(AutomationLaneId, u32, CurveType), // lane, tick, curve
    SelectLane(i8),                                  // +1/-1
    ClearLane(AutomationLaneId),
    ToggleRecording,
    RecordValue(AutomationTarget, f32),
    /// Delete automation points in tick range on a lane
    DeletePointsInRange(AutomationLaneId, u32, u32),
    /// Paste automation points at offset
    PastePoints(AutomationLaneId, u32, Vec<(u32, f32)>),
}

/// Session/file actions
#[derive(Debug, Clone, PartialEq)]
pub enum SessionAction {
    Save,
    Load,
    UpdateSession(MusicalSettings),
    UpdateSessionLive(MusicalSettings),
    OpenFileBrowser(FileSelectAction),
    ImportCustomSynthDef(PathBuf),
    ImportVstPlugin(PathBuf, VstPluginKind),
    AdjustHumanizeVelocity(f32),
    AdjustHumanizeTiming(f32),
    ToggleMasterMute,
}

/// Actions that can be returned from pane input handling
#[derive(Debug, Clone)]
pub enum Action {
    None,
    Quit,
    Nav(NavAction),
    Instrument(InstrumentAction),
    Mixer(MixerAction),
    PianoRoll(PianoRollAction),
    Arrangement(ArrangementAction),
    Server(ServerAction),
    Session(SessionAction),
    Sequencer(SequencerAction),
    Chopper(ChopperAction),
    Automation(AutomationAction),
    VstParam(VstParamAction),
    AudioFeedback(crate::audio::commands::AudioFeedback),
    /// Pane signals: pop piano_mode/pad_mode layer
    ExitPerformanceMode,
    /// Push a named layer onto the layer stack
    PushLayer(&'static str),
    /// Pop a named layer from the layer stack
    PopLayer(&'static str),
}

/// Result of toggling performance mode (piano/pad keyboard)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleResult {
    /// Pane doesn't support performance mode
    NotSupported,
    /// Piano keyboard was activated
    ActivatedPiano,
    /// Pad keyboard was activated
    ActivatedPad,
    /// Layout cycled (still in piano mode)
    CycledLayout,
    /// Performance mode was deactivated
    Deactivated,
}

/// Action to take when a file is selected in the file browser
#[derive(Debug, Clone, PartialEq)]
pub enum FileSelectAction {
    ImportCustomSynthDef,
    ImportVstInstrument,
    ImportVstEffect,
    LoadDrumSample(usize), // pad index
    LoadChopperSample,
    LoadPitchedSample(InstrumentId),
    LoadImpulseResponse(InstrumentId, usize), // instrument_id, effect_index
}

/// Navigation intent returned from dispatch — processed by the UI layer
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NavIntent {
    SwitchTo(&'static str),
    PushTo(&'static str),
    Pop,
    /// Pop only if the active pane matches the given id
    ConditionalPop(&'static str),
    /// Pop, falling back to SwitchTo if stack is empty
    PopOrSwitchTo(&'static str),
    /// Configure and push to the file browser
    OpenFileBrowser(FileSelectAction),
    /// Configure and push to the VST param pane for a specific target
    OpenVstParams(InstrumentId, VstTarget),
}

/// Status event returned from dispatch — forwarded to the server pane by the UI layer
#[derive(Debug, Clone)]
pub struct StatusEvent {
    pub status: ServerStatus,
    pub message: String,
    pub server_running: Option<bool>,
}

/// Result of dispatching an action — contains side effects for the UI layer to process
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub quit: bool,
    pub nav: Vec<NavIntent>,
    pub status: Vec<StatusEvent>,
    pub project_name: Option<String>,
    pub audio_dirty: AudioDirty,
}

impl Default for DispatchResult {
    fn default() -> Self {
        Self {
            quit: false,
            nav: Vec::new(),
            status: Vec::new(),
            project_name: None,
            audio_dirty: AudioDirty::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioDirty {
    pub instruments: bool,
    pub session: bool,
    pub piano_roll: bool,
    pub automation: bool,
    pub routing: bool,
    pub mixer_params: bool,
}

impl AudioDirty {
    pub fn all() -> Self {
        Self {
            instruments: true,
            session: true,
            piano_roll: true,
            automation: true,
            routing: true,
            mixer_params: true,
        }
    }

    pub fn any(&self) -> bool {
        self.instruments
            || self.session
            || self.piano_roll
            || self.automation
            || self.routing
            || self.mixer_params
    }

    pub fn merge(&mut self, other: AudioDirty) {
        self.instruments |= other.instruments;
        self.session |= other.session;
        self.piano_roll |= other.piano_roll;
        self.automation |= other.automation;
        self.routing |= other.routing;
        self.mixer_params |= other.mixer_params;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[allow(dead_code)]
impl DispatchResult {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn with_quit() -> Self {
        Self { quit: true, ..Self::default() }
    }

    pub fn with_nav(intent: NavIntent) -> Self {
        Self { nav: vec![intent], ..Self::default() }
    }

    pub fn with_status(status: ServerStatus, message: impl Into<String>) -> Self {
        Self {
            status: vec![StatusEvent { status, message: message.into(), server_running: None }],
            ..Self::default()
        }
    }

    pub fn push_nav(&mut self, intent: NavIntent) {
        self.nav.push(intent);
    }

    pub fn push_status(&mut self, status: ServerStatus, message: impl Into<String>) {
        self.status.push(StatusEvent { status, message: message.into(), server_running: None });
    }

    pub fn push_status_with_running(&mut self, status: ServerStatus, message: impl Into<String>, running: bool) {
        self.status.push(StatusEvent { status, message: message.into(), server_running: Some(running) });
    }

    pub fn mark_audio_dirty(&mut self, dirty: AudioDirty) {
        self.audio_dirty.merge(dirty);
    }

    pub fn merge(&mut self, other: DispatchResult) {
        self.quit = self.quit || other.quit;
        self.nav.extend(other.nav);
        self.status.extend(other.status);
        if other.project_name.is_some() {
            self.project_name = other.project_name;
        }
        self.audio_dirty.merge(other.audio_dirty);
    }
}
