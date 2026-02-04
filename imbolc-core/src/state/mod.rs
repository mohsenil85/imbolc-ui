pub mod arpeggiator;
pub mod automation;
pub mod arrangement;
pub mod clipboard;
pub mod custom_synthdef;
pub mod drum_sequencer;
pub mod grid;
pub mod instrument;
pub mod instrument_state;
pub mod midi_recording;
pub mod music;
pub mod param;
pub mod persistence;
pub mod piano_roll;
pub mod recent_projects;
pub mod sampler;
pub mod session;
pub mod undo;
pub mod vst_plugin;

pub use automation::AutomationTarget;
pub use arrangement::{ArrangementState, Clip, ClipId, ClipPlacement, PlayMode, PlacementId};
pub use clipboard::{Clipboard, ClipboardContents, ClipboardNote};
pub use custom_synthdef::{CustomSynthDef, CustomSynthDefRegistry, ParamSpec};
pub use instrument::*;
pub use instrument::{InstrumentSection, instrument_row_count, instrument_section_for_row, instrument_row_info};
pub use instrument_state::InstrumentState;
pub use param::{Param, ParamValue, adjust_freq_semitone, adjust_musical_step, is_freq_param};
pub use sampler::BufferId;
pub use session::{MixerSelection, MusicalSettings, SessionState, MAX_BUSES};
pub use undo::UndoHistory;
pub use vst_plugin::{VstParamSpec, VstPlugin, VstPluginId, VstPluginKind, VstPluginRegistry};

use std::path::PathBuf;

use crate::audio::ServerStatus;

/// State for a render-to-WAV operation in progress
#[derive(Debug, Clone)]
pub struct PendingRender {
    pub instrument_id: InstrumentId,
    pub path: PathBuf,
    pub was_looping: bool,
}

/// State for an export operation in progress (master bounce or stem export)
#[derive(Debug, Clone)]
pub struct PendingExport {
    pub kind: crate::audio::commands::ExportKind,
    pub was_looping: bool,
    pub paths: Vec<PathBuf>,
}

/// Keyboard layout configuration for key translation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyboardLayout {
    #[default]
    Qwerty,
    Colemak,
}

/// Real-time visualization data from audio analysis synths
#[derive(Debug, Clone)]
pub struct VisualizationState {
    /// 7-band spectrum analyzer amplitudes (60Hz, 150Hz, 400Hz, 1kHz, 2.5kHz, 6kHz, 15kHz)
    pub spectrum_bands: [f32; 7],
    /// Master output peak levels (left, right)
    pub peak_l: f32,
    pub peak_r: f32,
    /// Master output RMS levels (left, right)
    pub rms_l: f32,
    pub rms_r: f32,
    /// Oscilloscope ring buffer (recent peak samples at ~30Hz)
    pub scope_buffer: std::collections::VecDeque<f32>,
}

impl Default for VisualizationState {
    fn default() -> Self {
        Self {
            spectrum_bands: [0.0; 7],
            peak_l: 0.0,
            peak_r: 0.0,
            rms_l: 0.0,
            rms_r: 0.0,
            scope_buffer: std::collections::VecDeque::with_capacity(200),
        }
    }
}

/// Generation counters for async I/O results (ignore stale completions).
#[derive(Debug, Clone, Copy, Default)]
pub struct IoGeneration {
    pub save: u64,
    pub load: u64,
    pub import_synthdef: u64,
}

impl IoGeneration {
    pub fn next_save(&mut self) -> u64 {
        self.save = self.save.wrapping_add(1);
        self.save
    }

    pub fn next_load(&mut self) -> u64 {
        self.load = self.load.wrapping_add(1);
        self.load
    }

    pub fn next_import_synthdef(&mut self) -> u64 {
        self.import_synthdef = self.import_synthdef.wrapping_add(1);
        self.import_synthdef
    }
}

/// Top-level application state, owned by main.rs and passed to panes by reference.
pub struct AppState {
    pub session: SessionState,
    pub instruments: InstrumentState,
    pub clipboard: Clipboard,
    /// Path to a recently stopped recording, pending waveform load
    pub pending_recording_path: Option<std::path::PathBuf>,
    /// Pending render-to-WAV operation
    pub pending_render: Option<PendingRender>,
    /// Pending export operation (master bounce or stem export)
    pub pending_export: Option<PendingExport>,
    /// Export progress (0.0 to 1.0)
    pub export_progress: f32,
    pub keyboard_layout: KeyboardLayout,
    pub recording: bool,
    pub recording_secs: u64,
    pub automation_recording: bool,
    pub io_generation: IoGeneration,
    /// Real-time visualization data from audio analysis
    pub visualization: VisualizationState,
    pub recorded_waveform_peaks: Option<Vec<f32>>,
    /// Current project file path (None = untitled/new project)
    pub project_path: Option<PathBuf>,
    /// Whether state has changed since last save/load
    pub dirty: bool,
    /// Undo/redo history (owned by state so dispatch can manage it)
    pub undo_history: UndoHistory,
    /// Musical defaults used when creating new projects
    pub default_settings: MusicalSettings,
    /// Available MIDI input port names (updated by main loop)
    pub midi_port_names: Vec<String>,
    /// Currently connected MIDI port name
    pub midi_connected_port: Option<String>,
    /// Audio-owned playhead position (updated from AudioReadState each frame)
    pub audio_playhead: u32,
    /// Audio-owned BPM (updated from AudioReadState each frame)
    pub audio_bpm: f32,
    /// SC server status (updated from AudioReadState each frame)
    pub server_status: ServerStatus,
}

impl AppState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            session: SessionState::new(),
            instruments: InstrumentState::new(),
            clipboard: Clipboard::default(),
            pending_recording_path: None,
            pending_render: None,
            pending_export: None,
            export_progress: 0.0,
            keyboard_layout: KeyboardLayout::default(),
            recording: false,
            recording_secs: 0,
            automation_recording: false,
            io_generation: IoGeneration::default(),
            visualization: VisualizationState::default(),
            recorded_waveform_peaks: None,
            project_path: None,
            dirty: false,
            undo_history: UndoHistory::new(500),
            default_settings: MusicalSettings::default(),
            midi_port_names: Vec::new(),
            midi_connected_port: None,
            audio_playhead: 0,
            audio_bpm: 120.0,
            server_status: ServerStatus::Stopped,
        }
    }

    pub fn new_with_defaults(defaults: MusicalSettings) -> Self {
        Self {
            session: SessionState::new_with_defaults(defaults.clone()),
            instruments: InstrumentState::new(),
            clipboard: Clipboard::default(),
            pending_recording_path: None,
            pending_render: None,
            pending_export: None,
            export_progress: 0.0,
            keyboard_layout: KeyboardLayout::default(),
            recording: false,
            recording_secs: 0,
            automation_recording: false,
            io_generation: IoGeneration::default(),
            visualization: VisualizationState::default(),
            recorded_waveform_peaks: None,
            project_path: None,
            dirty: false,
            undo_history: UndoHistory::new(500),
            default_settings: defaults,
            midi_port_names: Vec::new(),
            midi_connected_port: None,
            audio_playhead: 0,
            audio_bpm: 120.0,
            server_status: ServerStatus::Stopped,
        }
    }

    /// Add an instrument, with custom synthdef param setup and piano roll track auto-creation.
    pub fn add_instrument(&mut self, source: SourceType) -> InstrumentId {
        let id = self.instruments.add_instrument(source);

        // For custom synthdefs, set params from registry
        if let SourceType::Custom(custom_id) = source {
            if let Some(synthdef) = self.session.custom_synthdefs.get(custom_id) {
                if let Some(inst) = self.instruments.instrument_mut(id) {
                    inst.name = format!("{}-{}", synthdef.synthdef_name, id);
                    inst.source_params = synthdef
                        .params
                        .iter()
                        .map(|p| param::Param {
                            name: p.name.clone(),
                            value: param::ParamValue::Float(p.default),
                            min: p.min,
                            max: p.max,
                        })
                        .collect();
                }
            }
        }

        // For VST instruments, set name from registry
        if let SourceType::Vst(vst_id) = source {
            if let Some(plugin) = self.session.vst_plugins.get(vst_id) {
                if let Some(inst) = self.instruments.instrument_mut(id) {
                    inst.name = format!("{}-{}", plugin.name.to_lowercase(), id);
                    inst.source_params = plugin
                        .params
                        .iter()
                        .map(|p| param::Param {
                            name: p.name.clone(),
                            value: param::ParamValue::Float(p.default),
                            min: 0.0,
                            max: 1.0,
                        })
                        .collect();
                }
            }
        }

        // Always add a piano roll track for every instrument
        self.session.piano_roll.add_track(id);

        id
    }

    /// Remove an instrument and its piano roll track.
    pub fn remove_instrument(&mut self, id: InstrumentId) {
        self.instruments.remove_instrument(id);
        self.session.piano_roll.remove_track(id);
        self.session.automation.remove_lanes_for_instrument(id);
        self.session.arrangement.remove_instrument_data(id);
    }

    /// Compute effective mute for an instrument, considering solo state and master mute.
    pub fn effective_instrument_mute(&self, inst: &Instrument) -> bool {
        if self.instruments.any_instrument_solo() {
            !inst.solo
        } else {
            inst.mute || self.session.master_mute
        }
    }

    /// Move mixer selection left/right
    pub fn mixer_move(&mut self, delta: i8) {
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Instrument(idx) => {
                let new_idx = (idx as i32 + delta as i32)
                    .clamp(0, self.instruments.instruments.len().saturating_sub(1) as i32)
                    as usize;
                MixerSelection::Instrument(new_idx)
            }
            MixerSelection::Bus(id) => {
                let new_id = (id as i8 + delta).clamp(1, MAX_BUSES as i8) as u8;
                MixerSelection::Bus(new_id)
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Jump to first (1) or last (-1) in current section
    pub fn mixer_jump(&mut self, direction: i8) {
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Instrument(_) => {
                if direction > 0 {
                    MixerSelection::Instrument(0)
                } else {
                    MixerSelection::Instrument(self.instruments.instruments.len().saturating_sub(1))
                }
            }
            MixerSelection::Bus(_) => {
                if direction > 0 {
                    MixerSelection::Bus(1)
                } else {
                    MixerSelection::Bus(MAX_BUSES as u8)
                }
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Cycle output target for the selected instrument
    pub fn mixer_cycle_output(&mut self) {
        if let MixerSelection::Instrument(idx) = self.session.mixer_selection {
            if let Some(inst) = self.instruments.instruments.get_mut(idx) {
                inst.output_target = match inst.output_target {
                    OutputTarget::Master => OutputTarget::Bus(1),
                    OutputTarget::Bus(n) if n < MAX_BUSES as u8 => OutputTarget::Bus(n + 1),
                    OutputTarget::Bus(_) => OutputTarget::Master,
                };
            }
        }
    }

    /// Cycle output target backwards for the selected instrument
    pub fn mixer_cycle_output_reverse(&mut self) {
        if let MixerSelection::Instrument(idx) = self.session.mixer_selection {
            if let Some(inst) = self.instruments.instruments.get_mut(idx) {
                inst.output_target = match inst.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_instrument_clears_automation_lanes() {
        let mut state = AppState::new();
        let instrument_id = state.add_instrument(SourceType::Saw);

        assert_eq!(state.session.piano_roll.track_order.len(), 1);
        assert_eq!(state.session.piano_roll.track_order[0], instrument_id);

        state
            .session
            .automation
            .add_lane(AutomationTarget::InstrumentLevel(instrument_id));
        state
            .session
            .automation
            .add_lane(AutomationTarget::InstrumentPan(instrument_id));

        assert_eq!(
            state.session.automation.lanes_for_instrument(instrument_id).len(),
            2
        );

        state.remove_instrument(instrument_id);

        assert!(state
            .session
            .automation
            .lanes_for_instrument(instrument_id)
            .is_empty());
        assert!(state.session.piano_roll.track_order.is_empty());
    }

    #[test]
    fn effective_instrument_mute_no_solo() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        let inst = &state.instruments.instruments[0];
        // Not muted, no solo, no master mute
        assert!(!state.effective_instrument_mute(inst));

        // Mute the instrument
        state.instruments.instruments[0].mute = true;
        let inst = &state.instruments.instruments[0];
        assert!(state.effective_instrument_mute(inst));

        // Unmute instrument but mute master
        state.instruments.instruments[0].mute = false;
        state.session.master_mute = true;
        let inst = &state.instruments.instruments[0];
        assert!(state.effective_instrument_mute(inst));
    }

    #[test]
    fn effective_instrument_mute_with_solo() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        state.add_instrument(SourceType::Sin);
        state.instruments.instruments[0].solo = true;

        let inst0 = &state.instruments.instruments[0];
        assert!(!state.effective_instrument_mute(inst0)); // soloed — not muted

        let inst1 = &state.instruments.instruments[1];
        assert!(state.effective_instrument_mute(inst1)); // not soloed — muted
    }

    #[test]
    fn mixer_move_clamps_instrument() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        state.add_instrument(SourceType::Sin);
        state.session.mixer_selection = MixerSelection::Instrument(0);

        state.mixer_move(-1);
        assert!(matches!(state.session.mixer_selection, MixerSelection::Instrument(0)));

        state.mixer_move(10);
        assert!(matches!(state.session.mixer_selection, MixerSelection::Instrument(1)));
    }

    #[test]
    fn mixer_move_clamps_bus() {
        let mut state = AppState::new();
        state.session.mixer_selection = MixerSelection::Bus(1);
        state.mixer_move(-1);
        assert!(matches!(state.session.mixer_selection, MixerSelection::Bus(1)));

        state.mixer_move(100);
        assert!(matches!(state.session.mixer_selection, MixerSelection::Bus(8)));
    }

    #[test]
    fn mixer_jump() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        state.add_instrument(SourceType::Sin);
        state.session.mixer_selection = MixerSelection::Instrument(0);

        state.mixer_jump(-1); // jump to last
        assert!(matches!(state.session.mixer_selection, MixerSelection::Instrument(1)));

        state.mixer_jump(1); // jump to first
        assert!(matches!(state.session.mixer_selection, MixerSelection::Instrument(0)));
    }

    #[test]
    fn mixer_cycle_output() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        state.session.mixer_selection = MixerSelection::Instrument(0);

        assert_eq!(state.instruments.instruments[0].output_target, OutputTarget::Master);
        state.mixer_cycle_output();
        assert_eq!(state.instruments.instruments[0].output_target, OutputTarget::Bus(1));

        // Cycle through all buses back to Master (Bus(1)..Bus(8)..Master = 8 more cycles)
        for _ in 1..=MAX_BUSES {
            state.mixer_cycle_output();
        }
        assert_eq!(state.instruments.instruments[0].output_target, OutputTarget::Master);
    }

    #[test]
    fn mixer_cycle_output_reverse() {
        let mut state = AppState::new();
        state.add_instrument(SourceType::Saw);
        state.session.mixer_selection = MixerSelection::Instrument(0);

        state.mixer_cycle_output_reverse();
        assert_eq!(state.instruments.instruments[0].output_target, OutputTarget::Bus(MAX_BUSES as u8));
        state.mixer_cycle_output_reverse();
        assert_eq!(state.instruments.instruments[0].output_target, OutputTarget::Bus(MAX_BUSES as u8 - 1));
    }

    #[test]
    fn add_instrument_creates_piano_roll_track() {
        let mut state = AppState::new();
        let id = state.add_instrument(SourceType::Saw);
        assert_eq!(state.session.piano_roll.track_order.len(), 1);
        assert!(state.session.piano_roll.tracks.contains_key(&id));
    }

    #[test]
    fn remove_instrument_cleans_up_all() {
        let mut state = AppState::new();
        let id = state.add_instrument(SourceType::Saw);
        state.session.automation.add_lane(AutomationTarget::InstrumentLevel(id));
        assert_eq!(state.session.automation.lanes.len(), 1);

        state.remove_instrument(id);
        assert!(state.session.piano_roll.track_order.is_empty());
        assert!(state.session.automation.lanes.is_empty());
    }
}
