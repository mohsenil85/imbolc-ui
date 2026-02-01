use crate::audio::AudioHandle;
use crate::state::AppState;
use crate::action::{DispatchResult, InstrumentAction, NavIntent};

pub(super) fn dispatch_instrument(
    action: &InstrumentAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    match action {
        InstrumentAction::Add(source_type) => {
            state.add_instrument(*source_type);
            let mut result = DispatchResult::with_nav(NavIntent::SwitchTo("instrument"));
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
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    audio.push_active_note(instrument_id, pitch, 240);
                }
            }
            DispatchResult::none()
        }
        InstrumentAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let _ = audio.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                        audio.push_active_note(instrument_id, pitch, 240);
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
                            if audio.is_running() {
                                let _ = audio.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument_id,
                                    pad.slice_start, pad.slice_end,
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
        InstrumentAction::AddEffect(_, _)
        | InstrumentAction::RemoveEffect(_, _)
        | InstrumentAction::MoveEffect(_, _, _)
        | InstrumentAction::SetFilter(_, _) => {
            // Reserved for future direct dispatch (currently handled inside InstrumentEditPane)
            let mut result = DispatchResult::none();
            result.audio_dirty.instruments = true;
            result.audio_dirty.routing = true;
            result
        }
    }
}
