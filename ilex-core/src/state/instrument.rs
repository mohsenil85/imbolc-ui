use super::custom_synthdef::{CustomSynthDefId, CustomSynthDefRegistry};
use super::drum_sequencer::DrumSequencerState;
use super::param::{Param, ParamValue};
use super::sampler::SamplerConfig;
use super::vst_plugin::{VstPluginId, VstPluginRegistry};

pub type InstrumentId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Saw,
    Sin,
    Sqr,
    Tri,
    Noise,
    Pulse,
    SuperSaw,
    Sync,
    Ring,
    FBSin,
    FM,
    PhaseMod,
    Pluck,
    Formant,
    Gendy,
    Chaos,
    Additive,
    Wavetable,
    AudioIn,
    BusIn,
    PitchedSampler,
    Kit,
    Custom(CustomSynthDefId),
    Vst(VstPluginId),
}

impl SourceType {
    pub fn name(&self) -> &'static str {
        match self {
            SourceType::Saw => "Saw",
            SourceType::Sin => "Sine",
            SourceType::Sqr => "Square",
            SourceType::Tri => "Triangle",
            SourceType::Noise => "Noise",
            SourceType::Pulse => "Pulse",
            SourceType::SuperSaw => "SuperSaw",
            SourceType::Sync => "Sync",
            SourceType::Ring => "Ring Mod",
            SourceType::FBSin => "FB Sine",
            SourceType::FM => "FM",
            SourceType::PhaseMod => "Phase Mod",
            SourceType::Pluck => "Pluck",
            SourceType::Formant => "Formant",
            SourceType::Gendy => "Gendy",
            SourceType::Chaos => "Chaos",
            SourceType::Additive => "Additive",
            SourceType::Wavetable => "Wavetable",
            SourceType::AudioIn => "Audio In",
            SourceType::BusIn => "Bus In",
            SourceType::PitchedSampler => "Pitched Sampler",
            SourceType::Kit => "Kit",
            SourceType::Custom(_) => "Custom",
            SourceType::Vst(_) => "VST",
        }
    }

    /// Get display name, with custom synthdef name lookup
    pub fn display_name(&self, registry: &CustomSynthDefRegistry) -> String {
        match self {
            SourceType::Custom(id) => registry
                .get(*id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "Custom".to_string()),
            _ => self.name().to_string(),
        }
    }

    /// Get display name with VST plugin registry lookup
    pub fn display_name_vst(&self, custom_registry: &CustomSynthDefRegistry, vst_registry: &VstPluginRegistry) -> String {
        match self {
            SourceType::Vst(id) => vst_registry
                .get(*id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "VST".to_string()),
            _ => self.display_name(custom_registry),
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            SourceType::Saw => "saw",
            SourceType::Sin => "sin",
            SourceType::Sqr => "sqr",
            SourceType::Tri => "tri",
            SourceType::Noise => "noise",
            SourceType::Pulse => "pulse",
            SourceType::SuperSaw => "supersaw",
            SourceType::Sync => "sync",
            SourceType::Ring => "ring",
            SourceType::FBSin => "fbsin",
            SourceType::FM => "fm",
            SourceType::PhaseMod => "phasemod",
            SourceType::Pluck => "pluck",
            SourceType::Formant => "formant",
            SourceType::Gendy => "gendy",
            SourceType::Chaos => "chaos",
            SourceType::Additive => "additive",
            SourceType::Wavetable => "wavetable",
            SourceType::AudioIn => "audio_in",
            SourceType::BusIn => "bus_in",
            SourceType::PitchedSampler => "sample",
            SourceType::Kit => "kit",
            SourceType::Custom(_) => "custom",
            SourceType::Vst(_) => "vst",
        }
    }

    /// Get short name with custom synthdef lookup
    pub fn short_name_with_registry(&self, registry: &CustomSynthDefRegistry) -> String {
        match self {
            SourceType::Custom(id) => registry
                .get(*id)
                .map(|s| s.synthdef_name.clone())
                .unwrap_or_else(|| "custom".to_string()),
            _ => self.short_name().to_string(),
        }
    }

    /// Get short name with VST plugin registry lookup
    pub fn short_name_vst(&self, custom_registry: &CustomSynthDefRegistry, vst_registry: &VstPluginRegistry) -> String {
        match self {
            SourceType::Vst(id) => vst_registry
                .get(*id)
                .map(|p| p.name.to_lowercase())
                .unwrap_or_else(|| "vst".to_string()),
            _ => self.short_name_with_registry(custom_registry),
        }
    }

    /// Get the SuperCollider synthdef name (static for built-ins)
    pub fn synth_def_name(&self) -> &'static str {
        match self {
            SourceType::Saw => "ilex_saw",
            SourceType::Sin => "ilex_sin",
            SourceType::Sqr => "ilex_sqr",
            SourceType::Tri => "ilex_tri",
            SourceType::Noise => "ilex_noise",
            SourceType::Pulse => "ilex_pulse",
            SourceType::SuperSaw => "ilex_supersaw",
            SourceType::Sync => "ilex_sync",
            SourceType::Ring => "ilex_ring",
            SourceType::FBSin => "ilex_fbsin",
            SourceType::FM => "ilex_fm",
            SourceType::PhaseMod => "ilex_phasemod",
            SourceType::Pluck => "ilex_pluck",
            SourceType::Formant => "ilex_formant",
            SourceType::Gendy => "ilex_gendy",
            SourceType::Chaos => "ilex_chaos",
            SourceType::Additive => "ilex_additive",
            SourceType::Wavetable => "ilex_wavetable",
            SourceType::AudioIn => "ilex_audio_in",
            SourceType::BusIn => "ilex_bus_in",
            SourceType::PitchedSampler => "ilex_sampler",
            SourceType::Kit => "ilex_sampler_oneshot",
            SourceType::Custom(_) => "ilex_saw", // Fallback, use synth_def_name_with_registry instead
            SourceType::Vst(_) => "ilex_vst_instrument",
        }
    }

    /// Get the SuperCollider synthdef name with custom synthdef lookup
    pub fn synth_def_name_with_registry(&self, registry: &CustomSynthDefRegistry) -> String {
        match self {
            SourceType::Custom(id) => registry
                .get(*id)
                .map(|s| s.synthdef_name.clone())
                .unwrap_or_else(|| "ilex_saw".to_string()),
            SourceType::Vst(_) => "ilex_vst_instrument".to_string(),
            _ => self.synth_def_name().to_string(),
        }
    }

    pub fn default_params(&self) -> Vec<Param> {
        match self {
            SourceType::Noise => vec![
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "color".to_string(), value: ParamValue::Int(0), min: 0.0, max: 4.0 },
                Param { name: "density".to_string(), value: ParamValue::Float(20.0), min: 1.0, max: 100.0 },
            ],
            SourceType::Pulse => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "width".to_string(), value: ParamValue::Float(0.5), min: 0.01, max: 0.99 },
            ],
            SourceType::SuperSaw => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "detune".to_string(), value: ParamValue::Float(0.3), min: 0.0, max: 1.0 },
                Param { name: "mix".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
            ],
            SourceType::Sync => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "sync_ratio".to_string(), value: ParamValue::Float(2.0), min: 1.0, max: 8.0 },
            ],
            SourceType::Ring => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "mod_ratio".to_string(), value: ParamValue::Float(2.0), min: 0.1, max: 16.0 },
                Param { name: "mod_depth".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
            ],
            SourceType::FBSin => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "feedback".to_string(), value: ParamValue::Float(0.0), min: 0.0, max: 3.0 },
            ],
            SourceType::FM => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "ratio".to_string(), value: ParamValue::Float(2.0), min: 0.25, max: 16.0 },
                Param { name: "index".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 20.0 },
            ],
            SourceType::PhaseMod => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "ratio".to_string(), value: ParamValue::Float(2.0), min: 0.25, max: 16.0 },
                Param { name: "index".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 10.0 },
            ],
            SourceType::Pluck => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "decay".to_string(), value: ParamValue::Float(2.0), min: 0.1, max: 10.0 },
                Param { name: "coef".to_string(), value: ParamValue::Float(0.3), min: 0.0, max: 1.0 },
            ],
            SourceType::Formant => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "formant".to_string(), value: ParamValue::Float(800.0), min: 100.0, max: 5000.0 },
                Param { name: "bw".to_string(), value: ParamValue::Float(200.0), min: 10.0, max: 1000.0 },
            ],
            SourceType::Gendy => vec![
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "ampdist".to_string(), value: ParamValue::Int(1), min: 0.0, max: 6.0 },
                Param { name: "durdist".to_string(), value: ParamValue::Int(1), min: 0.0, max: 6.0 },
                Param { name: "minfreq".to_string(), value: ParamValue::Float(100.0), min: 20.0, max: 1000.0 },
                Param { name: "maxfreq".to_string(), value: ParamValue::Float(1000.0), min: 100.0, max: 10000.0 },
            ],
            SourceType::Chaos => vec![
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "model".to_string(), value: ParamValue::Int(0), min: 0.0, max: 1.0 },
                Param { name: "chaos_freq".to_string(), value: ParamValue::Float(8000.0), min: 20.0, max: 20000.0 },
                Param { name: "chaos_param".to_string(), value: ParamValue::Float(1.3), min: 0.0, max: 2.0 },
            ],
            SourceType::Additive => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "harmonics".to_string(), value: ParamValue::Int(8), min: 1.0, max: 32.0 },
                Param { name: "rolloff".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 2.0 },
            ],
            SourceType::Wavetable => vec![
                Param { name: "freq".to_string(), value: ParamValue::Float(440.0), min: 20.0, max: 20000.0 },
                Param { name: "amp".to_string(), value: ParamValue::Float(0.5), min: 0.0, max: 1.0 },
                Param { name: "position".to_string(), value: ParamValue::Float(0.0), min: 0.0, max: 1.0 },
            ],
            SourceType::AudioIn => vec![
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
            SourceType::BusIn => vec![
                Param {
                    name: "bus".to_string(),
                    value: ParamValue::Int(1),
                    min: 1.0,
                    max: 8.0,
                },
                Param {
                    name: "gain".to_string(),
                    value: ParamValue::Float(1.0),
                    min: 0.0,
                    max: 4.0,
                },
            ],
            SourceType::PitchedSampler => vec![
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
            SourceType::Kit => vec![], // Pads have their own levels
            SourceType::Custom(_) => vec![], // Use default_params_with_registry instead
            SourceType::Vst(_) => vec![], // VST params discovered from plugin
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

    /// Get default params with custom synthdef lookup
    #[allow(dead_code)]
    pub fn default_params_with_registry(&self, registry: &CustomSynthDefRegistry) -> Vec<Param> {
        match self {
            SourceType::Custom(id) => registry
                .get(*id)
                .map(|s| {
                    s.params
                        .iter()
                        .map(|p| Param {
                            name: p.name.clone(),
                            value: ParamValue::Float(p.default),
                            min: p.min,
                            max: p.max,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => self.default_params(),
        }
    }

    pub fn is_audio_input(&self) -> bool {
        matches!(self, SourceType::AudioIn)
    }

    pub fn is_sample(&self) -> bool {
        matches!(self, SourceType::PitchedSampler)
    }

    pub fn is_kit(&self) -> bool {
        matches!(self, SourceType::Kit)
    }

    pub fn is_bus_in(&self) -> bool {
        matches!(self, SourceType::BusIn)
    }

    #[allow(dead_code)]
    pub fn is_custom(&self) -> bool {
        matches!(self, SourceType::Custom(_))
    }

    #[allow(dead_code)]
    pub fn custom_id(&self) -> Option<CustomSynthDefId> {
        match self {
            SourceType::Custom(id) => Some(*id),
            _ => None,
        }
    }

    pub fn is_vst(&self) -> bool {
        matches!(self, SourceType::Vst(_))
    }

    #[allow(dead_code)]
    pub fn vst_id(&self) -> Option<VstPluginId> {
        match self {
            SourceType::Vst(id) => Some(*id),
            _ => None,
        }
    }

    /// Built-in oscillator types (excluding custom)
    pub fn all() -> Vec<SourceType> {
        vec![
            SourceType::Saw, SourceType::Sin, SourceType::Sqr, SourceType::Tri,
            SourceType::Noise, SourceType::Pulse, SourceType::SuperSaw, SourceType::Sync,
            SourceType::Ring, SourceType::FBSin, SourceType::FM, SourceType::PhaseMod,
            SourceType::Pluck, SourceType::Formant, SourceType::Gendy, SourceType::Chaos,
            SourceType::Additive, SourceType::Wavetable,
            SourceType::AudioIn, SourceType::BusIn, SourceType::PitchedSampler, SourceType::Kit,
        ]
    }

    /// All oscillator types including custom ones from registry
    #[allow(dead_code)]
    pub fn all_with_custom(registry: &CustomSynthDefRegistry) -> Vec<SourceType> {
        let mut types = Self::all();
        for synthdef in &registry.synthdefs {
            types.push(SourceType::Custom(synthdef.id));
        }
        types
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            FilterType::Lpf => "ilex_lpf",
            FilterType::Hpf => "ilex_hpf",
            FilterType::Bpf => "ilex_bpf",
        }
    }

    #[allow(dead_code)]
    pub fn all() -> Vec<FilterType> {
        vec![FilterType::Lpf, FilterType::Hpf, FilterType::Bpf]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    Delay,
    Reverb,
    Gate,
    TapeComp,
    SidechainComp,
    Vst(VstPluginId),
}

impl EffectType {
    pub fn name(&self) -> &'static str {
        match self {
            EffectType::Delay => "Delay",
            EffectType::Reverb => "Reverb",
            EffectType::Gate => "Gate",
            EffectType::TapeComp => "Tape Comp",
            EffectType::SidechainComp => "SC Comp",
            EffectType::Vst(_) => "VST",
        }
    }

    /// Get display name with VST plugin registry lookup
    pub fn display_name(&self, vst_registry: &VstPluginRegistry) -> String {
        match self {
            EffectType::Vst(id) => vst_registry
                .get(*id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "VST".to_string()),
            _ => self.name().to_string(),
        }
    }

    pub fn synth_def_name(&self) -> &'static str {
        match self {
            EffectType::Delay => "ilex_delay",
            EffectType::Reverb => "ilex_reverb",
            EffectType::Gate => "ilex_gate",
            EffectType::TapeComp => "ilex_tape_comp",
            EffectType::SidechainComp => "ilex_sc_comp",
            EffectType::Vst(_) => "ilex_vst_effect",
        }
    }

    pub fn is_vst(&self) -> bool {
        matches!(self, EffectType::Vst(_))
    }

    #[allow(dead_code)]
    pub fn vst_id(&self) -> Option<VstPluginId> {
        match self {
            EffectType::Vst(id) => Some(*id),
            _ => None,
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
            EffectType::TapeComp => vec![
                Param { name: "drive".to_string(), value: ParamValue::Float(1.5), min: 1.0, max: 8.0 },
                Param { name: "threshold".to_string(), value: ParamValue::Float(0.5), min: 0.01, max: 1.0 },
                Param { name: "ratio".to_string(), value: ParamValue::Float(3.0), min: 1.0, max: 20.0 },
                Param { name: "makeup".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 4.0 },
                Param { name: "mix".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 1.0 },
            ],
            EffectType::SidechainComp => vec![
                Param { name: "sc_bus".to_string(), value: ParamValue::Int(0), min: 0.0, max: 8.0 }, // 0=self, 1-8=mixer bus
                Param { name: "threshold".to_string(), value: ParamValue::Float(0.3), min: 0.01, max: 1.0 },
                Param { name: "ratio".to_string(), value: ParamValue::Float(4.0), min: 1.0, max: 20.0 },
                Param { name: "attack".to_string(), value: ParamValue::Float(0.01), min: 0.001, max: 0.5 },
                Param { name: "release".to_string(), value: ParamValue::Float(0.1), min: 0.01, max: 2.0 },
                Param { name: "mix".to_string(), value: ParamValue::Float(1.0), min: 0.0, max: 1.0 },
            ],
            EffectType::Vst(_) => vec![], // VST params discovered from plugin
        }
    }

    #[allow(dead_code)]
    pub fn all() -> Vec<EffectType> {
        vec![EffectType::Delay, EffectType::Reverb, EffectType::Gate, EffectType::TapeComp, EffectType::SidechainComp]
    }
}

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
pub struct EnvConfig {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self { attack: 0.01, decay: 0.1, sustain: 0.0, release: 0.3 }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    #[allow(dead_code)]
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
// then wire it up in AudioEngine::rebuild_instrument_routing.
//
// Implementation notes per target:
//   FilterCutoff   - DONE (filter SynthDefs have cutoff_mod_in)
//   FilterResonance- Add res_mod_in to filter SynthDefs
//   Amplitude      - Add amp_mod_in to oscillator SynthDefs, multiply with amp
//   Pitch          - Add freq_mod_in to oscillators, multiply freq by 2^(mod) for semitones
//   Pan            - Add pan_mod_in to ilex_output, add to pan and clip
//   PulseWidth     - Add width_mod_in to ilex_sqr only, add to pulse width
//   SampleRate     - Add rate_mod_in to ilex_sampler, multiply with rate
//   DelayTime      - Add time_mod_in to ilex_delay
//   DelayFeedback  - Add feedback_mod_in to ilex_delay
//   ReverbMix      - Add mix_mod_in to ilex_reverb
//   GateRate       - Add rate_mod_in to ilex_gate (meta-modulation!)
//   SendLevel      - Add level_mod_in to ilex_send
//   Detune         - Add detune_mod_in to oscillators, slight pitch offset
//   Attack         - Add attack_mod_in to oscillators (unusual but possible)
//   Release        - Add release_mod_in to oscillators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    #[allow(dead_code)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Instrument {
    pub id: InstrumentId,
    pub name: String,
    pub source: SourceType,
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
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
        }
    }
}
