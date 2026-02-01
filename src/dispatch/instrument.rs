use crate::audio::AudioEngine;
use crate::panes::InstrumentEditPane;
use crate::state::AppState;
use crate::ui::{InstrumentAction, PaneManager};

pub(super) fn dispatch_instrument(
    action: &InstrumentAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
) {
    match action {
        InstrumentAction::Add(osc_type) => {
            state.add_instrument(*osc_type);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
            panes.switch_to("instrument", &*state);
        }
        InstrumentAction::Delete(inst_id) => {
            let inst_id = *inst_id;
            state.remove_instrument(inst_id);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
        }
        InstrumentAction::Edit(id) => {
            let inst_data = state.instruments.instrument(*id).cloned();
            if let Some(inst) = inst_data {
                if let Some(edit) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
                    edit.set_instrument(&inst);
                }
                panes.switch_to("instrument_edit", &*state);
            }
        }
        InstrumentAction::Update(id) => {
            let id = *id;
            // Apply edits from instrument_edit pane back to the instrument
            let edits = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit")
                .map(|edit| {
                    let mut dummy = crate::state::instrument::Instrument::new(id, crate::state::SourceType::Saw);
                    edit.apply_to(&mut dummy);
                    dummy
                });
            if let Some(edited) = edits {
                if let Some(instrument) = state.instruments.instrument_mut(id) {
                    instrument.source = edited.source;
                    instrument.source_params = edited.source_params;
                    instrument.filter = edited.filter;
                    instrument.effects = edited.effects;
                    instrument.amp_envelope = edited.amp_envelope;
                    instrument.polyphonic = edited.polyphonic;
                    instrument.active = edited.active;
                }
            }
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
            // Don't switch pane - stay in edit
        }
        InstrumentAction::SetParam(instrument_id, ref param, value) => {
            // Update state
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                if let Some(p) = instrument.source_params.iter_mut().find(|p| p.name == *param) {
                    p.value = crate::state::ParamValue::Float(*value);
                }
            }
            // Update audio engine in real-time
            if audio_engine.is_running() {
                let _ = audio_engine.set_source_param(*instrument_id, param, *value);
            }
        }
        InstrumentAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    let duration_ticks = 240;
                    active_notes.push((instrument_id, pitch, duration_ticks));
                }
            }
        }
        InstrumentAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                        active_notes.push((instrument_id, pitch, 240));
                    }
                }
            }
        }
        InstrumentAction::Select(idx) => {
            if *idx < state.instruments.instruments.len() {
                state.instruments.selected = Some(*idx);
            }
        }
        InstrumentAction::SelectNext => {
            state.instruments.select_next();
        }
        InstrumentAction::SelectPrev => {
            state.instruments.select_prev();
        }
        InstrumentAction::SelectFirst => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(0);
            }
        }
        InstrumentAction::SelectLast => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(state.instruments.instruments.len() - 1);
            }
        }
        InstrumentAction::PlayDrumPad(pad_idx) => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                if let Some(seq) = &instrument.drum_sequencer {
                    if let Some(pad) = seq.pads.get(*pad_idx) {
                        if let (Some(buffer_id), instrument_id) = (pad.buffer_id, instrument.id) {
                            let amp = pad.level;
                            if audio_engine.is_running() {
                                let _ = audio_engine.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument_id,
                                    pad.slice_start, pad.slice_end,
                                );
                            }
                        }
                    }
                }
            }
        }
        InstrumentAction::LoadSampleResult(instrument_id, ref path) => {
            let instrument_id = *instrument_id;
            let path_str = path.to_string_lossy().to_string();
            let sample_name = path.file_stem()
                .map(|s| s.to_string_lossy().to_string());

            let buffer_id = state.instruments.next_sampler_buffer_id;
            state.instruments.next_sampler_buffer_id += 1;

            if audio_engine.is_running() {
                let _ = audio_engine.load_sample(buffer_id, &path_str);
            }

            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                if let Some(ref mut config) = instrument.sampler_config {
                    config.buffer_id = Some(buffer_id);
                    config.sample_name = sample_name;
                }
            }

            panes.pop(&*state);

            // Refresh the instrument edit pane with updated sample info
            let inst_data = state.instruments.instrument(instrument_id).cloned();
            if let Some(inst) = inst_data {
                if let Some(edit) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
                    let saved_row = edit.selected_row;
                    edit.set_instrument(&inst);
                    edit.selected_row = saved_row;
                }
            }
        }
        InstrumentAction::AddEffect(_, _)
        | InstrumentAction::RemoveEffect(_, _)
        | InstrumentAction::MoveEffect(_, _, _)
        | InstrumentAction::SetFilter(_, _) => {
            // Reserved for future direct dispatch (currently handled inside InstrumentEditPane)
        }
    }
}
