use crate::audio::AudioHandle;
use crate::state::AppState;
use crate::state::automation::AutomationTarget;
use crate::action::{DispatchResult, InstrumentAction, NavIntent, VstTarget};

use super::automation::record_automation_point;

pub(super) fn dispatch_instrument(
    action: &InstrumentAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    match action {
        InstrumentAction::Add(source_type) => {
            state.add_instrument(*source_type);
            let mut result = DispatchResult::with_nav(NavIntent::SwitchTo("instrument_edit"));
            result.audio_dirty.instruments = true;
            result.audio_dirty.piano_roll = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::Delete(inst_id) => {
            let inst_id = *inst_id;
            state.remove_instrument(inst_id);
            let mut result = if state.instruments.instruments.is_empty() {
                DispatchResult::with_nav(NavIntent::SwitchTo("add"))
            } else {
                DispatchResult::none()
            };
            result.audio_dirty.instruments = true;
            result.audio_dirty.piano_roll = true;
            result.audio_dirty.automation = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::Edit(id) => {
            state.instruments.editing_instrument_id = Some(*id);
            DispatchResult::with_nav(NavIntent::SwitchTo("instrument_edit"))
        }
        InstrumentAction::Update(update) => {
            if let Some(instrument) = state.instruments.instrument_mut(update.id) {
                instrument.source = update.source.clone();
                instrument.source_params = update.source_params.clone();
                instrument.filter = update.filter.clone();
                instrument.eq = update.eq.clone();
                instrument.effects = update.effects.clone();
                instrument.amp_envelope = update.amp_envelope.clone();
                instrument.polyphonic = update.polyphonic;
                instrument.active = update.active;
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::SetParam(instrument_id, ref param, value) => {
            // Update state
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                if let Some(p) = instrument.source_params.iter_mut().find(|p| p.name == *param) {
                    p.value = crate::state::ParamValue::Float(*value);
                }
            }
            // Update audio engine in real-time
            if audio.is_running() {
                let _ = audio.set_source_param(*instrument_id, param, *value);
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            let instrument_info: Option<(u32, Option<crate::state::arpeggiator::ChordShape>)> =
                state.instruments.selected_instrument().map(|s| (s.id, s.chord_shape));

            if let Some((instrument_id, chord_shape)) = instrument_info {
                if audio.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let pitches = match chord_shape {
                        Some(shape) => shape.expand(pitch),
                        None => vec![pitch],
                    };
                    for p in &pitches {
                        let _ = audio.spawn_voice(instrument_id, *p, vel_f, 0.0, &state.instruments, &state.session);
                        audio.push_active_note(instrument_id, *p, 240);
                    }
                }
            }
            DispatchResult::none()
        }
        InstrumentAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let instrument_info: Option<(u32, Option<crate::state::arpeggiator::ChordShape>)> =
                state.instruments.selected_instrument().map(|s| (s.id, s.chord_shape));

            if let Some((instrument_id, chord_shape)) = instrument_info {
                if audio.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let expanded = match chord_shape {
                            Some(shape) => shape.expand(pitch),
                            None => vec![pitch],
                        };
                        for p in &expanded {
                            let _ = audio.spawn_voice(instrument_id, *p, vel_f, 0.0, &state.instruments, &state.session);
                            audio.push_active_note(instrument_id, *p, 240);
                        }
                    }
                }
            }
            DispatchResult::none()
        }
        InstrumentAction::Select(idx) => {
            if *idx < state.instruments.instruments.len() {
                state.instruments.selected = Some(*idx);
            }
            DispatchResult::none()
        }
        InstrumentAction::SelectNext => {
            state.instruments.select_next();
            DispatchResult::none()
        }
        InstrumentAction::SelectPrev => {
            state.instruments.select_prev();
            DispatchResult::none()
        }
        InstrumentAction::SelectFirst => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(0);
            }
            DispatchResult::none()
        }
        InstrumentAction::SelectLast => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(state.instruments.instruments.len() - 1);
            }
            DispatchResult::none()
        }
        InstrumentAction::PlayDrumPad(pad_idx) => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                if let Some(seq) = &instrument.drum_sequencer {
                    if let Some(pad) = seq.pads.get(*pad_idx) {
                        if let (Some(buffer_id), instrument_id) = (pad.buffer_id, instrument.id) {
                            let amp = pad.level;
                            let pitch_rate = 2.0_f32.powf(pad.pitch as f32 / 12.0);
                            let rate = if pad.reverse { -pitch_rate } else { pitch_rate };
                            if audio.is_running() {
                                let _ = audio.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument_id,
                                    pad.slice_start, pad.slice_end, rate,
                                );
                            }
                        }
                    }
                }
            }
            DispatchResult::none()
        }
        InstrumentAction::LoadSampleResult(instrument_id, ref path) => {
            let instrument_id = *instrument_id;
            let path_str = path.to_string_lossy().to_string();
            let sample_name = path.file_stem()
                .map(|s| s.to_string_lossy().to_string());

            let buffer_id = state.instruments.next_sampler_buffer_id;
            state.instruments.next_sampler_buffer_id += 1;

            if audio.is_running() {
                let _ = audio.load_sample(buffer_id, &path_str);
            }

            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                if let Some(ref mut config) = instrument.sampler_config {
                    config.buffer_id = Some(buffer_id);
                    config.sample_name = sample_name;
                }
            }

            let mut result = DispatchResult::with_nav(NavIntent::Pop);
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::AddEffect(id, ref effect_type) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                instrument.effects.push(crate::state::EffectSlot::new(*effect_type));
            }
            let mut result = DispatchResult::with_nav(NavIntent::Pop);
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::RemoveEffect(id, idx) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if *idx < instrument.effects.len() {
                    instrument.effects.remove(*idx);
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::MoveEffect(id, idx, direction) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                let new_idx = (*idx as i8 + direction).max(0) as usize;
                if *idx < instrument.effects.len() && new_idx < instrument.effects.len() {
                    instrument.effects.swap(*idx, new_idx);
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::SetFilter(id, filter_type) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                instrument.filter = filter_type.map(crate::state::FilterConfig::new);
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::ToggleEffectBypass(id, idx) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if let Some(effect) = instrument.effects.get_mut(*idx) {
                    effect.enabled = !effect.enabled;
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::ToggleFilter(id) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if instrument.filter.is_some() {
                    instrument.filter = None;
                } else {
                    instrument.filter = Some(crate::state::FilterConfig::new(crate::state::FilterType::Lpf));
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::CycleFilterType(id) => {
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if let Some(ref mut filter) = instrument.filter {
                    filter.filter_type = match filter.filter_type {
                        crate::state::FilterType::Lpf => crate::state::FilterType::Hpf,
                        crate::state::FilterType::Hpf => crate::state::FilterType::Bpf,
                        crate::state::FilterType::Bpf => crate::state::FilterType::Lpf,
                    };
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::AdjustFilterCutoff(id, delta) => {
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if let Some(ref mut filter) = instrument.filter {
                    filter.cutoff.value = (filter.cutoff.value + delta * filter.cutoff.max * 0.02)
                        .clamp(filter.cutoff.min, filter.cutoff.max);
                    if state.automation_recording && state.session.piano_roll.playing {
                        let target = AutomationTarget::FilterCutoff(instrument.id);
                        record_target = Some((target.clone(), target.normalize_value(filter.cutoff.value)));
                    }
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
            result
        }
        InstrumentAction::AdjustFilterResonance(id, delta) => {
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if let Some(ref mut filter) = instrument.filter {
                    filter.resonance.value = (filter.resonance.value + delta * 0.05)
                        .clamp(filter.resonance.min, filter.resonance.max);
                    if state.automation_recording && state.session.piano_roll.playing {
                        let target = AutomationTarget::FilterResonance(instrument.id);
                        record_target = Some((target.clone(), target.normalize_value(filter.resonance.value)));
                    }
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
            result
        }
        InstrumentAction::AdjustEffectParam(id, effect_idx, param_idx, delta) => {
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            if let Some(instrument) = state.instruments.instrument_mut(*id) {
                if let Some(effect) = instrument.effects.get_mut(*effect_idx) {
                    if let Some(param) = effect.params.get_mut(*param_idx) {
                        let range = param.max - param.min;
                        match &mut param.value {
                            crate::state::ParamValue::Float(v) => {
                                *v = (*v + delta * range * 0.02).clamp(param.min, param.max);
                                if state.automation_recording && state.session.piano_roll.playing {
                                    let target = AutomationTarget::EffectParam(instrument.id, *effect_idx, *param_idx);
                                    record_target = Some((target.clone(), target.normalize_value(*v)));
                                }
                            }
                            crate::state::ParamValue::Int(v) => {
                                *v = (*v + (delta * range * 0.02) as i32).clamp(param.min as i32, param.max as i32);
                            }
                            crate::state::ParamValue::Bool(b) => {
                                *b = !*b;
                            }
                        }
                    }
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
            result
        }
        InstrumentAction::ToggleArp(id) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.arpeggiator.enabled = !inst.arpeggiator.enabled;
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::CycleArpDirection(id) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.arpeggiator.direction = inst.arpeggiator.direction.next();
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::CycleArpRate(id) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.arpeggiator.rate = inst.arpeggiator.rate.next();
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::AdjustArpOctaves(id, delta) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.arpeggiator.octaves = (inst.arpeggiator.octaves as i8 + delta).clamp(1, 4) as u8;
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::AdjustArpGate(id, delta) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.arpeggiator.gate = (inst.arpeggiator.gate + delta).clamp(0.1, 1.0);
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::CycleChordShape(id) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.chord_shape = Some(match inst.chord_shape {
                    Some(shape) => shape.next(),
                    None => crate::state::arpeggiator::ChordShape::Major,
                });
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::ClearChordShape(id) => {
            if let Some(inst) = state.instruments.instrument_mut(*id) {
                inst.chord_shape = None;
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result
        }
        InstrumentAction::LoadIRResult(instrument_id, effect_idx, ref path) => {
            let instrument_id = *instrument_id;
            let effect_idx = *effect_idx;
            let path_str = path.to_string_lossy().to_string();

            let buffer_id = state.instruments.next_sampler_buffer_id;
            state.instruments.next_sampler_buffer_id += 1;

            if audio.is_running() {
                let _ = audio.load_sample(buffer_id, &path_str);
            }

            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                // Update the ir_buffer param on the convolution reverb effect
                if let Some(effect) = instrument.effects.get_mut(effect_idx) {
                    if effect.effect_type == crate::state::EffectType::ConvolutionReverb {
                        for p in &mut effect.params {
                            if p.name == "ir_buffer" {
                                p.value = crate::state::param::ParamValue::Int(buffer_id as i32);
                            }
                        }
                    }
                }
                instrument.convolution_ir_path = Some(path_str);
            }

            let mut result = DispatchResult::with_nav(NavIntent::Pop);
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
        InstrumentAction::OpenVstEffectParams(instrument_id, effect_idx) => {
            DispatchResult::with_nav(NavIntent::OpenVstParams(*instrument_id, VstTarget::Effect(*effect_idx)))
        }
        InstrumentAction::SetEqParam(instrument_id, band_idx, ref param_name, value) => {
            let instrument_id = *instrument_id;
            let band_idx = *band_idx;
            let value = *value;
            let mut record_target: Option<(AutomationTarget, f32)> = None;

            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                if let Some(ref mut eq) = instrument.eq {
                    if let Some(band) = eq.bands.get_mut(band_idx) {
                        match param_name.as_str() {
                            "freq" => band.freq = value.clamp(20.0, 20000.0),
                            "gain" => band.gain = value.clamp(-24.0, 24.0),
                            "q" => band.q = value.clamp(0.1, 10.0),
                            "on" => band.enabled = value > 0.5,
                            _ => {}
                        }
                        if state.automation_recording && state.session.piano_roll.playing {
                            let param_idx = match param_name.as_str() {
                                "freq" => Some(0),
                                "gain" => Some(1),
                                "q" => Some(2),
                                _ => None,
                            };
                            if let Some(pi) = param_idx {
                                let target = AutomationTarget::EqBandParam(instrument.id, band_idx, pi);
                                record_target = Some((target.clone(), target.normalize_value(value)));
                            }
                        }
                    }
                }
            }

            // Send real-time param update to audio engine
            if audio.is_running() {
                let sc_param = format!("b{}_{}", band_idx, param_name);
                let sc_value = if param_name == "q" { 1.0 / value } else { value };
                let _ = audio.set_eq_param(instrument_id, &sc_param, sc_value);
            }

            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
            result
        }
        InstrumentAction::ToggleEq(instrument_id) => {
            let instrument_id = *instrument_id;
            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                if instrument.eq.is_some() {
                    instrument.eq = None;
                } else {
                    instrument.eq = Some(crate::state::EqConfig::default());
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
    }
}
