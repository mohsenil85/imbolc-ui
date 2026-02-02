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

// All LFO targets are wired: each target has a corresponding *_mod_in param
// in the relevant SynthDef, connected via routing.rs (routing-level targets)
// or voices.rs (voice-level targets).
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
