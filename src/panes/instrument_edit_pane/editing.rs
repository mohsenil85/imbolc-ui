use super::{InstrumentEditPane, Section};
use crate::state::{Param, ParamValue};
use crate::ui::{Action, InstrumentAction, InstrumentUpdate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdjustMode {
    Tiny,
    Normal,
    Big,
    Musical,
}

/// Check if a parameter name represents a frequency-type parameter
fn is_freq_param(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("freq") || lower.contains("cutoff") || lower.contains("formant") || lower.contains("bw")
}

/// Move a frequency value by one semitone up or down
fn adjust_freq_semitone(value: f32, increase: bool, tuning_a4: f32, min: f32, max: f32) -> f32 {
    let midi = 69.0 + 12.0 * (value / tuning_a4).ln() / 2.0_f32.ln();
    let new_midi = if increase { midi.round() + 1.0 } else { midi.round() - 1.0 };
    (tuning_a4 * 2.0_f32.powf((new_midi - 69.0) / 12.0)).clamp(min, max)
}

/// Snap to nearest "nice" step based on param range
fn adjust_musical_step(value: f32, increase: bool, min: f32, max: f32) -> f32 {
    let range = max - min;
    let step = if range <= 1.0 {
        0.1
    } else if range <= 10.0 {
        0.5
    } else if range <= 100.0 {
        1.0
    } else {
        10.0
    };
    let snapped = (value / step).round() * step;
    let new_val = if increase { snapped + step } else { snapped - step };
    new_val.clamp(min, max)
}

impl InstrumentEditPane {
    pub(super) fn adjust_value(&mut self, increase: bool, big: bool) {
        let mode = if big { AdjustMode::Big } else { AdjustMode::Normal };
        self.adjust_value_with_mode(increase, mode, 440.0);
    }

    pub(super) fn adjust_value_with_mode(&mut self, increase: bool, mode: AdjustMode, tuning_a4: f32) {
        let (section, local_idx) = self.row_info(self.selected_row);
        let fraction = match mode {
            AdjustMode::Tiny => 0.01,
            AdjustMode::Normal => 0.05,
            AdjustMode::Big => 0.10,
            AdjustMode::Musical => 0.05, // fallback, overridden per-section
        };

        match section {
            Section::Source => {
                let param_idx = if self.source.is_sample() {
                    if local_idx == 0 { return; } // sample name row — not adjustable
                    local_idx - 1
                } else {
                    local_idx
                };
                if let Some(param) = self.source_params.get_mut(param_idx) {
                    if mode == AdjustMode::Musical {
                        adjust_param_musical(param, increase, tuning_a4);
                    } else {
                        adjust_param(param, increase, fraction);
                    }
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    match local_idx {
                        0 => {} // type - use 't' to cycle
                        1 => {
                            if mode == AdjustMode::Musical {
                                f.cutoff.value = adjust_freq_semitone(f.cutoff.value, increase, tuning_a4, f.cutoff.min, f.cutoff.max);
                            } else {
                                let range = f.cutoff.max - f.cutoff.min;
                                let delta = range * fraction;
                                if increase { f.cutoff.value = (f.cutoff.value + delta).min(f.cutoff.max); }
                                else { f.cutoff.value = (f.cutoff.value - delta).max(f.cutoff.min); }
                            }
                        }
                        2 => {
                            if mode == AdjustMode::Musical {
                                f.resonance.value = adjust_musical_step(f.resonance.value, increase, f.resonance.min, f.resonance.max);
                            } else {
                                let range = f.resonance.max - f.resonance.min;
                                let delta = range * fraction;
                                if increase { f.resonance.value = (f.resonance.value + delta).min(f.resonance.max); }
                                else { f.resonance.value = (f.resonance.value - delta).max(f.resonance.min); }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Section::Effects => {
                if let Some(effect) = self.effects.get_mut(local_idx) {
                    if let Some(param) = effect.params.first_mut() {
                        if mode == AdjustMode::Musical {
                            adjust_param_musical(param, increase, tuning_a4);
                        } else {
                            adjust_param(param, increase, fraction);
                        }
                    }
                }
            }
            Section::Lfo => {
                match local_idx {
                    0 => {} // enabled - use 'l' to toggle
                    1 => {
                        // rate: 0.1 to 32 Hz
                        let delta = match mode {
                            AdjustMode::Tiny => 0.1,
                            AdjustMode::Musical => 1.0,
                            AdjustMode::Big => 2.0,
                            AdjustMode::Normal => 0.5,
                        };
                        if increase { self.lfo.rate = (self.lfo.rate + delta).min(32.0); }
                        else { self.lfo.rate = (self.lfo.rate - delta).max(0.1); }
                    }
                    2 => {
                        // depth: 0 to 1
                        let delta = match mode {
                            AdjustMode::Tiny => 0.01,
                            AdjustMode::Musical => 0.1,
                            _ => fraction,
                        };
                        if increase { self.lfo.depth = (self.lfo.depth + delta).min(1.0); }
                        else { self.lfo.depth = (self.lfo.depth - delta).max(0.0); }
                    }
                    3 => {} // shape/target - use 's'/'m' to cycle
                    _ => {}
                }
            }
            Section::Envelope => {
                let delta = match mode {
                    AdjustMode::Tiny => 0.01,
                    AdjustMode::Musical => 0.1,
                    AdjustMode::Normal => 0.05,
                    AdjustMode::Big => 0.1,
                };
                let val = match local_idx {
                    0 => &mut self.amp_envelope.attack,
                    1 => &mut self.amp_envelope.decay,
                    2 => &mut self.amp_envelope.sustain,
                    3 => &mut self.amp_envelope.release,
                    _ => return,
                };
                if increase { *val = (*val + delta).min(if local_idx == 2 { 1.0 } else { 5.0 }); }
                else { *val = (*val - delta).max(0.0); }
            }
        }
    }

    pub(super) fn emit_update(&self) -> Action {
        if let Some(id) = self.instrument_id {
            Action::Instrument(InstrumentAction::Update(Box::new(InstrumentUpdate {
                id,
                source: self.source,
                source_params: self.source_params.clone(),
                filter: self.filter.clone(),
                eq: self.eq.clone(),
                effects: self.effects.clone(),
                amp_envelope: self.amp_envelope.clone(),
                polyphonic: self.polyphonic,
                active: self.active,
            })))
        } else {
            Action::None
        }
    }

    /// Set current parameter to its minimum (zero) value
    pub(super) fn zero_current_param(&mut self) {
        let (section, local_idx) = self.row_info(self.selected_row);

        match section {
            Section::Source => {
                let param_idx = if self.source.is_sample() {
                    if local_idx == 0 { return; }
                    local_idx - 1
                } else {
                    local_idx
                };
                if let Some(param) = self.source_params.get_mut(param_idx) {
                    zero_param(param);
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    match local_idx {
                        0 => {} // type - can't zero
                        1 => f.cutoff.value = f.cutoff.min,
                        2 => f.resonance.value = f.resonance.min,
                        _ => {}
                    }
                }
            }
            Section::Effects => {
                if let Some(effect) = self.effects.get_mut(local_idx) {
                    if let Some(param) = effect.params.first_mut() {
                        zero_param(param);
                    }
                }
            }
            Section::Lfo => {
                match local_idx {
                    0 => self.lfo.enabled = false,
                    1 => self.lfo.rate = 0.1,
                    2 => self.lfo.depth = 0.0,
                    3 => {} // shape/target - can't zero
                    _ => {}
                }
            }
            Section::Envelope => {
                match local_idx {
                    0 => self.amp_envelope.attack = 0.0,
                    1 => self.amp_envelope.decay = 0.0,
                    2 => self.amp_envelope.sustain = 0.0,
                    3 => self.amp_envelope.release = 0.0,
                    _ => {}
                }
            }
        }
    }

    /// Set all parameters in the current section to their minimum values
    pub(super) fn zero_current_section(&mut self) {
        let section = self.current_section();

        match section {
            Section::Source => {
                for param in &mut self.source_params {
                    zero_param(param);
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    f.cutoff.value = f.cutoff.min;
                    f.resonance.value = f.resonance.min;
                }
            }
            Section::Effects => {
                for effect in &mut self.effects {
                    for param in &mut effect.params {
                        zero_param(param);
                    }
                }
            }
            Section::Lfo => {
                self.lfo.enabled = false;
                self.lfo.rate = 0.1;
                self.lfo.depth = 0.0;
            }
            Section::Envelope => {
                self.amp_envelope.attack = 0.0;
                self.amp_envelope.decay = 0.0;
                self.amp_envelope.sustain = 0.0;
                self.amp_envelope.release = 0.0;
            }
        }
    }

    /// Get current parameter value as a string for pre-filling text edit
    pub(super) fn current_value_string(&self) -> String {
        let (section, local_idx) = self.row_info(self.selected_row);
        match section {
            Section::Source => {
                let param_idx = if self.source.is_sample() {
                    if local_idx == 0 { return String::new(); }
                    local_idx - 1
                } else {
                    local_idx
                };
                if let Some(param) = self.source_params.get(param_idx) {
                    match &param.value {
                        ParamValue::Float(v) => format!("{:.2}", v),
                        ParamValue::Int(v) => format!("{}", v),
                        ParamValue::Bool(v) => format!("{}", v),
                    }
                } else {
                    String::new()
                }
            }
            Section::Filter => {
                if let Some(ref f) = self.filter {
                    match local_idx {
                        1 => format!("{:.2}", f.cutoff.value),
                        2 => format!("{:.2}", f.resonance.value),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                }
            }
            Section::Envelope => {
                match local_idx {
                    0 => format!("{:.2}", self.amp_envelope.attack),
                    1 => format!("{:.2}", self.amp_envelope.decay),
                    2 => format!("{:.2}", self.amp_envelope.sustain),
                    3 => format!("{:.2}", self.amp_envelope.release),
                    _ => String::new(),
                }
            }
            _ => String::new(),
        }
    }
}

fn adjust_param_musical(param: &mut Param, increase: bool, tuning_a4: f32) {
    match &mut param.value {
        ParamValue::Float(ref mut v) => {
            if is_freq_param(&param.name) {
                *v = adjust_freq_semitone(*v, increase, tuning_a4, param.min, param.max);
            } else {
                *v = adjust_musical_step(*v, increase, param.min, param.max);
            }
        }
        ParamValue::Int(ref mut v) => {
            let range = param.max - param.min;
            let step = if range <= 10.0 { 1 } else if range <= 100.0 { 5 } else { 10 };
            if increase { *v = (*v + step).min(param.max as i32); }
            else { *v = (*v - step).max(param.min as i32); }
        }
        ParamValue::Bool(ref mut v) => { *v = !*v; }
    }
}

pub(super) fn adjust_param(param: &mut Param, increase: bool, fraction: f32) {
    let range = param.max - param.min;
    match &mut param.value {
        ParamValue::Float(ref mut v) => {
            let delta = range * fraction;
            if increase { *v = (*v + delta).min(param.max); }
            else { *v = (*v - delta).max(param.min); }
        }
        ParamValue::Int(ref mut v) => {
            let delta = ((range * fraction) as i32).max(1);
            if increase { *v = (*v + delta).min(param.max as i32); }
            else { *v = (*v - delta).max(param.min as i32); }
        }
        ParamValue::Bool(ref mut v) => { *v = !*v; }
    }
}

pub(super) fn zero_param(param: &mut Param) {
    match &mut param.value {
        ParamValue::Float(ref mut v) => *v = param.min,
        ParamValue::Int(ref mut v) => *v = param.min as i32,
        ParamValue::Bool(ref mut v) => *v = false,
    }
}
