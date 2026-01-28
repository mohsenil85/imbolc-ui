use serde::{Deserialize, Serialize};

use super::param::{Param, ParamValue};

pub type StripId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OscType {
    Saw,
    Sin,
    Sqr,
    Tri,
}

impl OscType {
    pub fn name(&self) -> &'static str {
        match self {
            OscType::Saw => "Saw",
            OscType::Sin => "Sine",
            OscType::Sqr => "Square",
            OscType::Tri => "Triangle",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            OscType::Saw => "saw",
            OscType::Sin => "sin",
            OscType::Sqr => "sqr",
            OscType::Tri => "tri",
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            OscType::Saw => "tuidaw_saw",
            OscType::Sin => "tuidaw_sin",
            OscType::Sqr => "tuidaw_sqr",
            OscType::Tri => "tuidaw_tri",
        }
    }

    pub fn default_params() -> Vec<Param> {
        vec![
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
        ]
    }

    pub fn all() -> Vec<OscType> {
        vec![OscType::Saw, OscType::Sin, OscType::Sqr, OscType::Tri]
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
}

impl EffectType {
    pub fn name(&self) -> &'static str {
        match self {
            EffectType::Delay => "Delay",
            EffectType::Reverb => "Reverb",
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            EffectType::Delay => "tuidaw_delay",
            EffectType::Reverb => "tuidaw_reverb",
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
        }
    }

    pub fn all() -> Vec<EffectType> {
        vec![EffectType::Delay, EffectType::Reverb]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    pub rate: f32,
    pub depth: f32,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self { rate: 1.0, depth: 0.5 }
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
}

impl Strip {
    pub fn new(id: StripId, source: OscType) -> Self {
        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        Self {
            id,
            name: format!("{}-{}", source.short_name(), id),
            source,
            source_params: OscType::default_params(),
            filter: None,
            effects: Vec::new(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            has_track: true,
            level: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
            output_target: OutputTarget::Master,
            sends,
        }
    }
}
