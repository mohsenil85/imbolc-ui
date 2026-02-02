use super::ModulatedParam;

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
            FilterType::Lpf => "imbolc_lpf",
            FilterType::Hpf => "imbolc_hpf",
            FilterType::Bpf => "imbolc_bpf",
        }
    }

    #[allow(dead_code)]
    pub fn all() -> Vec<FilterType> {
        vec![FilterType::Lpf, FilterType::Hpf, FilterType::Bpf]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqBandType {
    LowShelf,
    Peaking,
    HighShelf,
}

impl EqBandType {
    pub fn name(&self) -> &'static str {
        match self {
            EqBandType::LowShelf => "LS",
            EqBandType::Peaking => "PK",
            EqBandType::HighShelf => "HS",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EqBand {
    pub band_type: EqBandType,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub enabled: bool,
}

pub const EQ_BAND_COUNT: usize = 12;

#[derive(Debug, Clone)]
pub struct EqConfig {
    pub bands: [EqBand; EQ_BAND_COUNT],
    pub enabled: bool,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            bands: [
                EqBand { band_type: EqBandType::LowShelf, freq: 40.0,    gain: 0.0, q: 0.7, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 80.0,    gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 160.0,   gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 320.0,   gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 640.0,   gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 1200.0,  gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 2500.0,  gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 5000.0,  gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 8000.0,  gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 12000.0, gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::Peaking,  freq: 16000.0, gain: 0.0, q: 1.0, enabled: true },
                EqBand { band_type: EqBandType::HighShelf, freq: 18000.0, gain: 0.0, q: 0.7, enabled: true },
            ],
            enabled: true,
        }
    }
}
