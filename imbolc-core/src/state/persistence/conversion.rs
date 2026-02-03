use super::super::automation::AutomationTarget;
use super::super::instrument::*;

// --- AutomationTarget serialization helpers ---

pub(super) fn serialize_automation_target(
    target: &AutomationTarget,
) -> (&'static str, InstrumentId, Option<i32>, Option<i32>) {
    match target {
        AutomationTarget::InstrumentLevel(id) => ("instrument_level", *id, None, None),
        AutomationTarget::InstrumentPan(id) => ("instrument_pan", *id, None, None),
        AutomationTarget::FilterCutoff(id) => ("filter_cutoff", *id, None, None),
        AutomationTarget::FilterResonance(id) => ("filter_resonance", *id, None, None),
        AutomationTarget::EffectParam(id, fx, param) => {
            ("effect_param", *id, Some(*fx as i32), Some(*param as i32))
        }
        AutomationTarget::SampleRate(id) => ("sample_rate", *id, None, None),
        AutomationTarget::SampleAmp(id) => ("sample_amp", *id, None, None),
        AutomationTarget::LfoRate(id) => ("lfo_rate", *id, None, None),
        AutomationTarget::LfoDepth(id) => ("lfo_depth", *id, None, None),
        AutomationTarget::EnvelopeAttack(id) => ("envelope_attack", *id, None, None),
        AutomationTarget::EnvelopeDecay(id) => ("envelope_decay", *id, None, None),
        AutomationTarget::EnvelopeSustain(id) => ("envelope_sustain", *id, None, None),
        AutomationTarget::EnvelopeRelease(id) => ("envelope_release", *id, None, None),
        AutomationTarget::SendLevel(id, idx) => ("send_level", *id, Some(*idx as i32), None),
        AutomationTarget::BusLevel(bus) => ("bus_level", 0, Some(*bus as i32), None),
        AutomationTarget::Bpm => ("bpm", 0, None, None),
        AutomationTarget::VstParam(id, idx) => ("vst_param", *id, Some(*idx as i32), None),
        AutomationTarget::EqBandParam(id, band, param) => {
            ("eq_band_param", *id, Some(*band as i32), Some(*param as i32))
        }
    }
}

pub(super) fn deserialize_automation_target(
    target_type: &str,
    instrument_id: InstrumentId,
    effect_idx: Option<i32>,
    param_idx: Option<i32>,
) -> Option<AutomationTarget> {
    match target_type {
        "instrument_level" => Some(AutomationTarget::InstrumentLevel(instrument_id)),
        "instrument_pan" => Some(AutomationTarget::InstrumentPan(instrument_id)),
        "filter_cutoff" => Some(AutomationTarget::FilterCutoff(instrument_id)),
        "filter_resonance" => Some(AutomationTarget::FilterResonance(instrument_id)),
        "effect_param" => {
            let fx = effect_idx.unwrap_or(0) as u32;
            let param = param_idx.unwrap_or(0) as usize;
            Some(AutomationTarget::EffectParam(instrument_id, fx, param))
        }
        "sample_rate" => Some(AutomationTarget::SampleRate(instrument_id)),
        "sample_amp" => Some(AutomationTarget::SampleAmp(instrument_id)),
        "lfo_rate" => Some(AutomationTarget::LfoRate(instrument_id)),
        "lfo_depth" => Some(AutomationTarget::LfoDepth(instrument_id)),
        "envelope_attack" => Some(AutomationTarget::EnvelopeAttack(instrument_id)),
        "envelope_decay" => Some(AutomationTarget::EnvelopeDecay(instrument_id)),
        "envelope_sustain" => Some(AutomationTarget::EnvelopeSustain(instrument_id)),
        "envelope_release" => Some(AutomationTarget::EnvelopeRelease(instrument_id)),
        "send_level" => {
            let idx = effect_idx.unwrap_or(0) as usize;
            Some(AutomationTarget::SendLevel(instrument_id, idx))
        }
        "bus_level" => {
            let bus = effect_idx.unwrap_or(1) as u8;
            Some(AutomationTarget::BusLevel(bus))
        }
        "bpm" => Some(AutomationTarget::Bpm),
        "vst_param" => {
            let idx = effect_idx.unwrap_or(0) as u32;
            Some(AutomationTarget::VstParam(instrument_id, idx))
        }
        "eq_band_param" => {
            let band = effect_idx.unwrap_or(0) as usize;
            let param = param_idx.unwrap_or(0) as usize;
            Some(AutomationTarget::EqBandParam(instrument_id, band, param))
        }
        _ => None,
    }
}

// --- Parse helpers ---

pub(super) fn parse_key(s: &str) -> super::super::music::Key {
    use super::super::music::Key;
    Key::ALL
        .iter()
        .find(|k| k.name() == s)
        .copied()
        .unwrap_or(Key::C)
}

pub(super) fn parse_scale(s: &str) -> super::super::music::Scale {
    use super::super::music::Scale;
    Scale::ALL
        .iter()
        .find(|sc| sc.name() == s)
        .copied()
        .unwrap_or(Scale::Major)
}

pub(super) fn parse_source_type(s: &str) -> SourceType {
    match s {
        "saw" => SourceType::Saw,
        "sin" => SourceType::Sin,
        "sqr" => SourceType::Sqr,
        "tri" => SourceType::Tri,
        "noise" => SourceType::Noise,
        "pulse" => SourceType::Pulse,
        "supersaw" => SourceType::SuperSaw,
        "sync" => SourceType::Sync,
        "ring" => SourceType::Ring,
        "fbsin" => SourceType::FBSin,
        "fm" => SourceType::FM,
        "phasemod" => SourceType::PhaseMod,
        "pluck" => SourceType::Pluck,
        "formant" => SourceType::Formant,
        "gendy" => SourceType::Gendy,
        "chaos" => SourceType::Chaos,
        "additive" => SourceType::Additive,
        "wavetable" => SourceType::Wavetable,
        "granular" => SourceType::Granular,
        "bowed" => SourceType::Bowed,
        "blown" => SourceType::Blown,
        "membrane" => SourceType::Membrane,
        "audio_in" => SourceType::AudioIn,
        "sample" | "sampler" | "pitched_sampler" => SourceType::PitchedSampler,
        "kit" | "drum" => SourceType::Kit,
        "bus_in" => SourceType::BusIn,
        other if other.starts_with("custom:") => {
            if let Ok(id) = other[7..].parse::<u32>() {
                SourceType::Custom(id)
            } else {
                SourceType::Saw
            }
        }
        other if other.starts_with("vst:") => {
            if let Ok(id) = other[4..].parse::<u32>() {
                SourceType::Vst(id)
            } else {
                SourceType::Saw
            }
        }
        _ => SourceType::Saw,
    }
}

pub(super) fn parse_filter_type(s: &str) -> FilterType {
    match s {
        "lpf" => FilterType::Lpf,
        "hpf" => FilterType::Hpf,
        "bpf" => FilterType::Bpf,
        "notch" => FilterType::Notch,
        "comb" => FilterType::Comb,
        "allpass" => FilterType::Allpass,
        "vowel" => FilterType::Vowel,
        "resdrive" => FilterType::ResDrive,
        _ => FilterType::Lpf,
    }
}

pub(super) fn parse_effect_type(s: &str) -> EffectType {
    match s {
        "delay" => EffectType::Delay,
        "reverb" => EffectType::Reverb,
        "gate" => EffectType::Gate,
        "tapecomp" => EffectType::TapeComp,
        "sidechaincomp" => EffectType::SidechainComp,
        "chorus" => EffectType::Chorus,
        "flanger" => EffectType::Flanger,
        "phaser" => EffectType::Phaser,
        "tremolo" => EffectType::Tremolo,
        "distortion" => EffectType::Distortion,
        "bitcrusher" => EffectType::Bitcrusher,
        "wavefolder" => EffectType::Wavefolder,
        "saturator" => EffectType::Saturator,
        "tilteq" => EffectType::TiltEq,
        "stereowidener" => EffectType::StereoWidener,
        "freqshifter" => EffectType::FreqShifter,
        "limiter" => EffectType::Limiter,
        "pitchshifter" => EffectType::PitchShifter,
        "vinyl" => EffectType::Vinyl,
        "cabinet" => EffectType::Cabinet,
        "granulardelay" => EffectType::GranularDelay,
        "granularfreeze" => EffectType::GranularFreeze,
        "convolutionreverb" => EffectType::ConvolutionReverb,
        "vocoder" => EffectType::Vocoder,
        "ringmod" => EffectType::RingMod,
        "autopan" => EffectType::Autopan,
        "resonator" => EffectType::Resonator,
        "multibandcomp" => EffectType::MultibandComp,
        "paraeq" => EffectType::ParaEq,
        "spectralfreeze" => EffectType::SpectralFreeze,
        "glitch" => EffectType::Glitch,
        "leslie" => EffectType::Leslie,
        "springreverb" => EffectType::SpringReverb,
        "envfollower" => EffectType::EnvFollower,
        "midside" => EffectType::MidSide,
        "crossfader" => EffectType::Crossfader,
        other if other.starts_with("vst:") => {
            if let Ok(id) = other[4..].parse::<u32>() {
                EffectType::Vst(id)
            } else {
                EffectType::Delay
            }
        }
        _ => EffectType::Delay,
    }
}
