pub mod automation;
pub mod custom_synthdef;
pub mod midi_recording;
pub mod music;
pub mod param;
mod persistence;
pub mod piano_roll;
pub mod sampler;
pub mod strip;
pub mod strip_state;

pub use automation::{AutomationLane, AutomationLaneId, AutomationPoint, AutomationState, AutomationTarget, CurveType};
pub use custom_synthdef::{CustomSynthDef, CustomSynthDefId, CustomSynthDefRegistry, ParamSpec};
pub use midi_recording::{MidiCcMapping, MidiRecordingState, PitchBendConfig, RecordMode};
pub use param::{Param, ParamValue};
pub use piano_roll::PianoRollState;
pub use sampler::{BufferId, SampleBuffer, SamplerConfig, SampleRegistry, Slice, SliceId};
pub use strip::*;
pub use strip_state::{MixerSelection, StripState};

/// Top-level application state, owned by main.rs and passed to panes by reference.
pub struct AppState {
    pub strip: StripState,
    pub audio_in_waveform: Option<Vec<f32>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            strip: StripState::new(),
            audio_in_waveform: None,
        }
    }
}
