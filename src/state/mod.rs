pub mod automation;
pub mod midi_recording;
pub mod music;
pub mod param;
mod persistence;
pub mod piano_roll;
pub mod sampler;
pub mod strip;
pub mod strip_state;

pub use automation::{AutomationLane, AutomationLaneId, AutomationPoint, AutomationState, AutomationTarget, CurveType};
pub use midi_recording::{MidiCcMapping, MidiRecordingState, PitchBendConfig, RecordMode};
pub use param::{Param, ParamValue};
pub use piano_roll::PianoRollState;
pub use sampler::{BufferId, SampleBuffer, SamplerConfig, SampleRegistry, Slice, SliceId};
pub use strip::*;
pub use strip_state::{MixerSelection, StripState};
