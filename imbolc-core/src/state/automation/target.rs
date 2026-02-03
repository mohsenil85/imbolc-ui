#![allow(dead_code)]

use serde::{Serialize, Deserialize};

use crate::state::instrument::{EffectId, Instrument, InstrumentId, SourceType};
use crate::state::vst_plugin::VstPluginRegistry;

/// What parameter is being automated
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationTarget {
    /// Instrument output level
    InstrumentLevel(InstrumentId),
    /// Instrument pan
    InstrumentPan(InstrumentId),
    /// Filter cutoff frequency
    FilterCutoff(InstrumentId),
    /// Filter resonance
    FilterResonance(InstrumentId),
    /// Effect parameter (instrument_id, effect_id, param_index)
    EffectParam(InstrumentId, EffectId, usize),
    /// Sample playback rate (for scratching)
    SampleRate(InstrumentId),
    /// Sample amplitude
    SampleAmp(InstrumentId),
    /// LFO rate (0.1–32.0 Hz)
    LfoRate(InstrumentId),
    /// LFO depth (0.0–1.0)
    LfoDepth(InstrumentId),
    /// Envelope attack time (0.001–2.0 s)
    EnvelopeAttack(InstrumentId),
    /// Envelope decay time (0.001–2.0 s)
    EnvelopeDecay(InstrumentId),
    /// Envelope sustain level (0.0–1.0)
    EnvelopeSustain(InstrumentId),
    /// Envelope release time (0.001–5.0 s)
    EnvelopeRelease(InstrumentId),
    /// Send level (instrument_id, send_index, 0.0–1.0)
    SendLevel(InstrumentId, usize),
    /// Bus output level (bus 1-8, 0.0–1.0)
    BusLevel(u8),
    /// Global BPM (30.0–300.0)
    Bpm,
    /// VST plugin parameter (instrument_id, param_index, 0.0–1.0 normalized)
    VstParam(InstrumentId, u32),
    /// EQ band parameter (instrument_id, band_index 0–11, param: 0=freq 1=gain 2=q)
    EqBandParam(InstrumentId, usize, usize),
}

impl AutomationTarget {
    /// Get the instrument ID associated with this target (None for global targets)
    pub fn instrument_id(&self) -> Option<InstrumentId> {
        match self {
            AutomationTarget::InstrumentLevel(id) => Some(*id),
            AutomationTarget::InstrumentPan(id) => Some(*id),
            AutomationTarget::FilterCutoff(id) => Some(*id),
            AutomationTarget::FilterResonance(id) => Some(*id),
            AutomationTarget::EffectParam(id, _, _) => Some(*id),
            AutomationTarget::SampleRate(id) => Some(*id),
            AutomationTarget::SampleAmp(id) => Some(*id),
            AutomationTarget::LfoRate(id) => Some(*id),
            AutomationTarget::LfoDepth(id) => Some(*id),
            AutomationTarget::EnvelopeAttack(id) => Some(*id),
            AutomationTarget::EnvelopeDecay(id) => Some(*id),
            AutomationTarget::EnvelopeSustain(id) => Some(*id),
            AutomationTarget::EnvelopeRelease(id) => Some(*id),
            AutomationTarget::SendLevel(id, _) => Some(*id),
            AutomationTarget::BusLevel(_) => None,
            AutomationTarget::Bpm => None,
            AutomationTarget::VstParam(id, _) => Some(*id),
            AutomationTarget::EqBandParam(id, _, _) => Some(*id),
        }
    }

    /// Get a human-readable name for this target
    pub fn name(&self) -> String {
        match self {
            AutomationTarget::InstrumentLevel(_) => "Level".to_string(),
            AutomationTarget::InstrumentPan(_) => "Pan".to_string(),
            AutomationTarget::FilterCutoff(_) => "Filter Cutoff".to_string(),
            AutomationTarget::FilterResonance(_) => "Filter Resonance".to_string(),
            AutomationTarget::EffectParam(_, fx_idx, param_idx) => {
                format!("FX{} Param{}", fx_idx + 1, param_idx + 1)
            }
            AutomationTarget::SampleRate(_) => "Sample Rate".to_string(),
            AutomationTarget::SampleAmp(_) => "Sample Amp".to_string(),
            AutomationTarget::LfoRate(_) => "LFO Rate".to_string(),
            AutomationTarget::LfoDepth(_) => "LFO Depth".to_string(),
            AutomationTarget::EnvelopeAttack(_) => "Env Attack".to_string(),
            AutomationTarget::EnvelopeDecay(_) => "Env Decay".to_string(),
            AutomationTarget::EnvelopeSustain(_) => "Env Sustain".to_string(),
            AutomationTarget::EnvelopeRelease(_) => "Env Release".to_string(),
            AutomationTarget::SendLevel(_, idx) => format!("Send {}", idx + 1),
            AutomationTarget::BusLevel(bus) => format!("Bus {} Level", bus),
            AutomationTarget::Bpm => "BPM".to_string(),
            AutomationTarget::VstParam(_, idx) => format!("VST P{}", idx),
            AutomationTarget::EqBandParam(_, band, param) => {
                let param_name = match param {
                    0 => "Freq",
                    1 => "Gain",
                    _ => "Q",
                };
                format!("EQ B{} {}", band + 1, param_name)
            }
        }
    }

    /// Get a short name for compact display
    pub fn short_name(&self) -> &'static str {
        match self {
            AutomationTarget::InstrumentLevel(_) => "Level",
            AutomationTarget::InstrumentPan(_) => "Pan",
            AutomationTarget::FilterCutoff(_) => "FltCt",
            AutomationTarget::FilterResonance(_) => "FltRs",
            AutomationTarget::EffectParam(_, _, _) => "FX",
            AutomationTarget::SampleRate(_) => "SRate",
            AutomationTarget::SampleAmp(_) => "SAmp",
            AutomationTarget::LfoRate(_) => "LfoRt",
            AutomationTarget::LfoDepth(_) => "LfoDp",
            AutomationTarget::EnvelopeAttack(_) => "EnvA",
            AutomationTarget::EnvelopeDecay(_) => "EnvD",
            AutomationTarget::EnvelopeSustain(_) => "EnvS",
            AutomationTarget::EnvelopeRelease(_) => "EnvR",
            AutomationTarget::SendLevel(_, _) => "Send",
            AutomationTarget::BusLevel(_) => "BusLv",
            AutomationTarget::Bpm => "BPM",
            AutomationTarget::VstParam(_, _) => "VstP",
            AutomationTarget::EqBandParam(_, _, _) => "EqBd",
        }
    }

    /// Get all possible automation targets for an instrument
    pub fn targets_for_instrument(id: InstrumentId) -> Vec<AutomationTarget> {
        vec![
            AutomationTarget::InstrumentLevel(id),
            AutomationTarget::InstrumentPan(id),
            AutomationTarget::FilterCutoff(id),
            AutomationTarget::FilterResonance(id),
            AutomationTarget::LfoRate(id),
            AutomationTarget::LfoDepth(id),
            AutomationTarget::EnvelopeAttack(id),
            AutomationTarget::EnvelopeDecay(id),
            AutomationTarget::EnvelopeSustain(id),
            AutomationTarget::EnvelopeRelease(id),
        ]
    }

    /// Get context-aware automation targets for an instrument.
    /// Includes the static 10 targets plus context-dependent ones based on
    /// the instrument's effects, source type, VST plugins, and EQ.
    pub fn targets_for_instrument_context(inst: &Instrument, vst_registry: &VstPluginRegistry) -> Vec<AutomationTarget> {
        let id = inst.id;
        let mut targets = Self::targets_for_instrument(id);

        // EffectParam: one target per param for each non-VST effect
        for effect in &inst.effects {
            if effect.effect_type.is_vst() {
                continue;
            }
            for (param_idx, _param) in effect.params.iter().enumerate() {
                targets.push(AutomationTarget::EffectParam(id, effect.id, param_idx));
            }
        }

        // SampleRate + SampleAmp: only for sample-based sources
        if matches!(inst.source, SourceType::PitchedSampler | SourceType::Kit) {
            targets.push(AutomationTarget::SampleRate(id));
            targets.push(AutomationTarget::SampleAmp(id));
        }

        // VstParam: only for VST source instruments
        if let SourceType::Vst(vst_id) = inst.source {
            if let Some(plugin) = vst_registry.get(vst_id) {
                for param in &plugin.params {
                    targets.push(AutomationTarget::VstParam(id, param.index));
                }
            }
        }

        // EqBandParam: only when EQ is enabled (12 bands x 3 params = 36 targets)
        if inst.eq.is_some() {
            for band in 0..12 {
                for param in 0..3 {
                    targets.push(AutomationTarget::EqBandParam(id, band, param));
                }
            }
        }

        targets
    }

    /// Get a context-aware display name using instrument data for richer labels.
    /// Falls back to `name()` for targets that don't benefit from context.
    pub fn name_with_context(&self, inst: Option<&Instrument>, vst_registry: &VstPluginRegistry) -> String {
        match self {
            AutomationTarget::EffectParam(_, effect_id, param_idx) => {
                if let Some(inst) = inst {
                    if let Some(effect) = inst.effect_by_id(*effect_id) {
                        let effect_name = effect.effect_type.name();
                        if let Some(param) = effect.params.get(*param_idx) {
                            return format!("{} > {}", effect_name, param.name);
                        }
                    }
                }
                self.name()
            }
            AutomationTarget::VstParam(_, param_index) => {
                if let Some(inst) = inst {
                    if let SourceType::Vst(vst_id) = inst.source {
                        if let Some(plugin) = vst_registry.get(vst_id) {
                            if let Some(param) = plugin.params.iter().find(|p| p.index == *param_index) {
                                return format!("VST: {}", param.name);
                            }
                        }
                    }
                }
                self.name()
            }
            _ => self.name(),
        }
    }

    /// Normalize an actual parameter value to 0.0–1.0 based on this target's range
    pub fn normalize_value(&self, actual: f32) -> f32 {
        let (min, max) = self.default_range();
        if max > min { ((actual - min) / (max - min)).clamp(0.0, 1.0) } else { 0.5 }
    }

    /// Get the default min/max range for this target type
    pub fn default_range(&self) -> (f32, f32) {
        match self {
            AutomationTarget::InstrumentLevel(_) => (0.0, 1.0),
            AutomationTarget::InstrumentPan(_) => (-1.0, 1.0),
            AutomationTarget::FilterCutoff(_) => (20.0, 20000.0),
            AutomationTarget::FilterResonance(_) => (0.0, 1.0),
            AutomationTarget::EffectParam(_, _, _) => (0.0, 1.0),
            AutomationTarget::SampleRate(_) => (-2.0, 2.0),
            AutomationTarget::SampleAmp(_) => (0.0, 1.0),
            AutomationTarget::LfoRate(_) => (0.1, 32.0),
            AutomationTarget::LfoDepth(_) => (0.0, 1.0),
            AutomationTarget::EnvelopeAttack(_) => (0.001, 2.0),
            AutomationTarget::EnvelopeDecay(_) => (0.001, 2.0),
            AutomationTarget::EnvelopeSustain(_) => (0.0, 1.0),
            AutomationTarget::EnvelopeRelease(_) => (0.001, 5.0),
            AutomationTarget::SendLevel(_, _) => (0.0, 1.0),
            AutomationTarget::BusLevel(_) => (0.0, 1.0),
            AutomationTarget::Bpm => (30.0, 300.0),
            AutomationTarget::VstParam(_, _) => (0.0, 1.0),
            AutomationTarget::EqBandParam(_, _, param) => match param {
                0 => (20.0, 20000.0),  // freq
                1 => (-24.0, 24.0),    // gain
                _ => (0.1, 10.0),      // Q
            },
        }
    }
}
