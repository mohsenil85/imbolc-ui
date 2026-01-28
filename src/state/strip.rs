use serde::{Deserialize, Serialize};

use super::param::{Param, ParamValue};
use super::sampler::SamplerConfig;

pub type StripId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OscType {
    Saw,
    Sin,
    Sqr,
    Tri,
    AudioIn,
    Sampler,
}

impl OscType {
    pub fn name(&self) -> &'static str {
        match self {
            OscType::Saw => "Saw",
            OscType::Sin => "Sine",
            OscType::Sqr => "Square",
            OscType::Tri => "Triangle",
            OscType::AudioIn => "Audio In",
            OscType::Sampler => "Sampler",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            OscType::Saw => "saw",
            OscType::Sin => "sin",
            OscType::Sqr => "sqr",
            OscType::Tri => "tri",
            OscType::AudioIn => "audio_in",
            OscType::Sampler => "sampler",
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            OscType::Saw => "tuidaw_saw",
            OscType::Sin => "tuidaw_sin",
            OscType::Sqr => "tuidaw_sqr",
            OscType::Tri => "tuidaw_tri",
            OscType::AudioIn => "tuidaw_audio_in",
            OscType::Sampler => "tuidaw_sampler",
        }
    }

    pub fn default_params(&self) -> Vec<Param> {
        match self {
            OscType::AudioIn => vec![
                Param {
                    name: "gain".to_string(),
                    value: ParamValue::Float(1.0),
                    min: 0.0,
                    max: 4.0,
                },
                Param {
                    name: "channel".to_string(),
                    value: ParamValue::Int(0),
                    min: 0.0,
                    max: 7.0,
                },
                Param {
                    name: "test_tone".to_string(),
                    value: ParamValue::Float(0.0),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "test_freq".to_string(),
                    value: ParamValue::Float(440.0),
                    min: 20.0,
                    max: 2000.0,
                },
            ],
            OscType::Sampler => vec![
                Param {
                    name: "rate".to_string(),
                    value: ParamValue::Float(1.0),
                    min: -2.0,
                    max: 2.0,
                },
                Param {
                    name: "amp".to_string(),
                    value: ParamValue::Float(0.8),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "loop".to_string(),
                    value: ParamValue::Bool(false),
                    min: 0.0,
                    max: 1.0,
                },
            ],
            _ => vec![
                Param {
                    name: "freq".to_string(),
                    value: ParamValue::Float(440.0),
                    min: 20.0,
                    max: 20000.0,
                },
                Param {
                    name: "amp".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
            ],
        }
    }

    pub fn is_audio_input(&self) -> bool {
        matches!(self, OscType::AudioIn)
    }

    pub fn is_sampler(&self) -> bool {
        matches!(self, OscType::Sampler)
    }

    pub fn all() -> Vec<OscType> {
        vec![OscType::Saw, OscType::Sin, OscType::Sqr, OscType::Tri, OscType::AudioIn, OscType::Sampler]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    Lpf,
    Hpf,
    Bpf,
}

impl FilterType {
    pub fn name(&self) -> &'static str {
        match self {
            FilterType::Lpf => "Low-Pass",
            FilterType::Hpf => "High-Pass",
            FilterType::Bpf => "Band-Pass",
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            FilterType::Lpf => "tuidaw_lpf",
            FilterType::Hpf => "tuidaw_hpf",
            FilterType::Bpf => "tuidaw_bpf",
        }
    }

    pub fn all() -> Vec<FilterType> {
        vec![FilterType::Lpf, FilterType::Hpf, FilterType::Bpf]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectType {
    Delay,
    Reverb,
    Gate,
}

impl EffectType {
    pub fn name(&self) -> &'static str {
        match self {
            EffectType::Delay => "Delay",
            EffectType::Reverb => "Reverb",
            EffectType::Gate => "Gate",
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            EffectType::Delay => "tuidaw_delay",
            EffectType::Reverb => "tuidaw_reverb",
            EffectType::Gate => "tuidaw_gate",
        }
    }

    pub fn default_params(&self) -> Vec<Param> {
        match self {
            EffectType::Delay => vec![
                Param { name: "time".to_string(), value: ParamValue::Float(0.3), min: 0.0, max: 2.0 },
                Param { name: "feedback".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "mix".to_string(), value: ParamValue::Float(0.3), min: 0.0, max: 1.0 },
            ],
            EffectType::Reverb => vec![
                Param { name: "room".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "damp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "mix".to_string(), value: ParamValue::Float(0.3), min: 0.0, max: 1.0 },
            ],
            EffectType::Gate => vec![
                Param { name: "rate".to_string(), value: ParamValue::Float(4.0), min: 0.1, max: 32.0 },
                Param { name: "depth".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 1.0 },
                Param { name: "shape".to_string(), value: ParamValue::Int(1), min: 0.0, max: 2.0 }, // 0=sine, 1=square, 2=saw
            ],
        }
    }

    pub fn all() -> Vec<EffectType> {
        vec![EffectType::Delay, EffectType::Reverb, EffectType::Gate]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputTarget {
    Master,
    Bus(u8), // 1-8
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self::Master
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self { attack: 0.01, decay: 0.1, sustain: 0.7, release: 0.3 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulatedParam {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub mod_source: Option<ModSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModSource {
    Lfo(LfoConfig),
    Envelope(EnvConfig),
    StripParam(StripId, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoShape {
    Sine,
    Square,
    Saw,
    Triangle,
}

impl LfoShape {
    pub fn name(&self) -> &'static str {
        match self {
            LfoShape::Sine => "Sine",
            LfoShape::Square => "Square",
            LfoShape::Saw => "Saw",
            LfoShape::Triangle => "Triangle",
        }
    }

    pub fn index(&self) -> i32 {
        match self {
            LfoShape::Sine => 0,
            LfoShape::Square => 1,
            LfoShape::Saw => 2,
            LfoShape::Triangle => 3,
        }
    }

    pub fn all() -> Vec<LfoShape> {
        vec![LfoShape::Sine, LfoShape::Square, LfoShape::Saw, LfoShape::Triangle]
    }

    pub fn next(&self) -> LfoShape {
        match self {
            LfoShape::Sine => LfoShape::Square,
            LfoShape::Square => LfoShape::Saw,
            LfoShape::Saw => LfoShape::Triangle,
            LfoShape::Triangle => LfoShape::Sine,
        }
    }
}

// TODO: Currently only FilterCutoff is wired up in the audio engine.
// To implement each target, add a `*_mod_in` param to the relevant SynthDef,
// then wire it up in AudioEngine::rebuild_strip_routing.
//
// Implementation notes per target:
//   FilterCutoff   - DONE (filter SynthDefs have cutoff_mod_in)
//   FilterResonance- Add res_mod_in to filter SynthDefs
//   Amplitude      - Add amp_mod_in to oscillator SynthDefs, multiply with amp
//   Pitch          - Add freq_mod_in to oscillators, multiply freq by 2^(mod) for semitones
//   Pan            - Add pan_mod_in to tuidaw_output, add to pan and clip
//   PulseWidth     - Add width_mod_in to tuidaw_sqr only, add to pulse width
//   SampleRate     - Add rate_mod_in to tuidaw_sampler, multiply with rate
//   DelayTime      - Add time_mod_in to tuidaw_delay
//   DelayFeedback  - Add feedback_mod_in to tuidaw_delay
//   ReverbMix      - Add mix_mod_in to tuidaw_reverb
//   GateRate       - Add rate_mod_in to tuidaw_gate (meta-modulation!)
//   SendLevel      - Add level_mod_in to tuidaw_send
//   Detune         - Add detune_mod_in to oscillators, slight pitch offset
//   Attack         - Add attack_mod_in to oscillators (unusual but possible)
//   Release        - Add release_mod_in to oscillators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoTarget {
    FilterCutoff,
    FilterResonance,
    Amplitude,
    Pitch,
    Pan,
    PulseWidth,
    SampleRate,
    DelayTime,
    DelayFeedback,
    ReverbMix,
    GateRate,
    SendLevel,
    Detune,
    Attack,
    Release,
}

impl LfoTarget {
    pub fn name(&self) -> &'static str {
        match self {
            LfoTarget::FilterCutoff => "Flt Cut",
            LfoTarget::FilterResonance => "Flt Res",
            LfoTarget::Amplitude => "Amp",
            LfoTarget::Pitch => "Pitch",
            LfoTarget::Pan => "Pan",
            LfoTarget::PulseWidth => "PW",
            LfoTarget::SampleRate => "SmpRate",
            LfoTarget::DelayTime => "DlyTime",
            LfoTarget::DelayFeedback => "DlyFdbk",
            LfoTarget::ReverbMix => "RevMix",
            LfoTarget::GateRate => "GateRt",
            LfoTarget::SendLevel => "Send",
            LfoTarget::Detune => "Detune",
            LfoTarget::Attack => "Attack",
            LfoTarget::Release => "Release",
        }
    }

    pub fn all() -> Vec<LfoTarget> {
        vec![
            LfoTarget::FilterCutoff,
            LfoTarget::FilterResonance,
            LfoTarget::Amplitude,
            LfoTarget::Pitch,
            LfoTarget::Pan,
            LfoTarget::PulseWidth,
            LfoTarget::SampleRate,
            LfoTarget::DelayTime,
            LfoTarget::DelayFeedback,
            LfoTarget::ReverbMix,
            LfoTarget::GateRate,
            LfoTarget::SendLevel,
            LfoTarget::Detune,
            LfoTarget::Attack,
            LfoTarget::Release,
        ]
    }

    pub fn next(&self) -> LfoTarget {
        match self {
            LfoTarget::FilterCutoff => LfoTarget::FilterResonance,
            LfoTarget::FilterResonance => LfoTarget::Amplitude,
            LfoTarget::Amplitude => LfoTarget::Pitch,
            LfoTarget::Pitch => LfoTarget::Pan,
            LfoTarget::Pan => LfoTarget::PulseWidth,
            LfoTarget::PulseWidth => LfoTarget::SampleRate,
            LfoTarget::SampleRate => LfoTarget::DelayTime,
            LfoTarget::DelayTime => LfoTarget::DelayFeedback,
            LfoTarget::DelayFeedback => LfoTarget::ReverbMix,
            LfoTarget::ReverbMix => LfoTarget::GateRate,
            LfoTarget::GateRate => LfoTarget::SendLevel,
            LfoTarget::SendLevel => LfoTarget::Detune,
            LfoTarget::Detune => LfoTarget::Attack,
            LfoTarget::Attack => LfoTarget::Release,
            LfoTarget::Release => LfoTarget::FilterCutoff,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    pub enabled: bool,
    pub rate: f32,
    pub depth: f32,
    pub shape: LfoShape,
    pub target: LfoTarget,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rate: 2.0,
            depth: 0.5,
            shape: LfoShape::Sine,
            target: LfoTarget::FilterCutoff,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub filter_type: FilterType,
    pub cutoff: ModulatedParam,
    pub resonance: ModulatedParam,
}

impl FilterConfig {
    pub fn new(filter_type: FilterType) -> Self {
        Self {
            filter_type,
            cutoff: ModulatedParam { value: 1000.0, min: 20.0, max: 20000.0, mod_source: None },
            resonance: ModulatedParam { value: 0.5, min: 0.0, max: 1.0, mod_source: None },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectSlot {
    pub effect_type: EffectType,
    pub params: Vec<Param>,
    pub enabled: bool,
}

impl EffectSlot {
    pub fn new(effect_type: EffectType) -> Self {
        Self {
            params: effect_type.default_params(),
            effect_type,
            enabled: true,
        }
    }
}

pub const MAX_BUSES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strip {
    pub id: StripId,
    pub name: String,
    pub source: OscType,
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
    pub effects: Vec<EffectSlot>,
    pub lfo: LfoConfig,
    pub amp_envelope: EnvConfig,
    pub polyphonic: bool,
    pub has_track: bool,
    // Integrated mixer
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub output_target: OutputTarget,
    pub sends: Vec<MixerSend>,
    // Sampler configuration (only used when source is OscType::Sampler)
    pub sampler_config: Option<SamplerConfig>,
}

impl Strip {
    pub fn new(id: StripId, source: OscType) -> Self {
        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        // Audio input strips don't have piano roll tracks
        let has_track = !source.is_audio_input();
        // Sampler strips get a sampler config
        let sampler_config = if source.is_sampler() {
            Some(SamplerConfig::default())
        } else {
            None
        };
        Self {
            id,
            name: format!("{}-{}", source.short_name(), id),
            source,
            source_params: source.default_params(),
            filter: None,
            effects: Vec::new(),
            lfo: LfoConfig::default(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            has_track,
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
            output_target: OutputTarget::Master,
            sends,
            sampler_config,
        }
    }
}
