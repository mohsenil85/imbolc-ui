mod source_type;
mod filter;
mod effect;
mod lfo;
mod envelope;

pub use source_type::*;
pub use filter::*;
pub use effect::*;
pub use lfo::*;
pub use envelope::*;

use super::drum_sequencer::DrumSequencerState;
use super::param::Param;
use super::sampler::SamplerConfig;

pub type InstrumentId = u32;

pub const MAX_BUSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTarget {
    Master,
    Bus(u8), // 1-8
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self::Master
    }
}

#[derive(Debug, Clone)]
pub struct MixerSend {
    pub bus_id: u8,
    pub level: f32,
    pub enabled: bool,
}

impl MixerSend {
    pub fn new(bus_id: u8) -> Self {
        Self { bus_id, level: 0.0, enabled: false }
    }
}

#[derive(Debug, Clone)]
pub struct MixerBus {
    pub id: u8,
    pub name: String,
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
}

impl MixerBus {
    pub fn new(id: u8) -> Self {
        Self {
            id,
            name: format!("Bus {}", id),
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModulatedParam {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub mod_source: Option<ModSource>,
}

#[derive(Debug, Clone)]
pub enum ModSource {
    Lfo(LfoConfig),
    Envelope(EnvConfig),
    InstrumentParam(InstrumentId, String),
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub id: InstrumentId,
    pub name: String,
    pub source: SourceType,
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
    pub eq: Option<EqConfig>,
    pub effects: Vec<EffectSlot>,
    pub lfo: LfoConfig,
    pub amp_envelope: EnvConfig,
    pub polyphonic: bool,
    // Integrated mixer
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub active: bool,
    pub output_target: OutputTarget,
    pub sends: Vec<MixerSend>,
    // Sample configuration (only used when source is SourceType::PitchedSampler)
    pub sampler_config: Option<SamplerConfig>,
    // Kit sequencer (only used when source is SourceType::Kit)
    pub drum_sequencer: Option<DrumSequencerState>,
    // Per-instance VST parameter values: (param_index, normalized_value)
    pub vst_param_values: Vec<(u32, f32)>,
    // Path to saved VST plugin state file (.fxp)
    pub vst_state_path: Option<std::path::PathBuf>,
    /// Arpeggiator configuration
    pub arpeggiator: super::arpeggiator::ArpeggiatorConfig,
    /// Chord shape (None = single notes, Some = expand to chord)
    pub chord_shape: Option<super::arpeggiator::ChordShape>,
    /// Path to loaded impulse response file for convolution reverb
    pub convolution_ir_path: Option<String>,
    /// Layer group ID: instruments sharing the same group sound together
    pub layer_group: Option<u32>,
}

impl Instrument {
    pub fn new(id: InstrumentId, source: SourceType) -> Self {
        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        // Sample instruments get a sampler config
        let sampler_config = if source.is_sample() {
            Some(SamplerConfig::default())
        } else {
            None
        };
        // Kit instruments get a drum sequencer
        let drum_sequencer = if source.is_kit() {
            Some(DrumSequencerState::new())
        } else {
            None
        };
        Self {
            id,
            name: format!("{}-{}", source.short_name(), id),
            source,
            source_params: source.default_params(),
            filter: None,
            eq: None,
            effects: Vec::new(),
            lfo: LfoConfig::default(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
            active: !source.is_audio_input(),
            output_target: OutputTarget::Master,
            sends,
            sampler_config,
            drum_sequencer,
            vst_param_values: Vec::new(),
            vst_state_path: None,
            arpeggiator: super::arpeggiator::ArpeggiatorConfig::default(),
            chord_shape: None,
            convolution_ir_path: None,
            layer_group: None,
        }
    }
}
