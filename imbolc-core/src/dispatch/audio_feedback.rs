use crate::action::{DispatchResult, NavIntent, VstTarget};
use crate::audio::commands::AudioFeedback;
use crate::audio::AudioHandle;
use crate::state::AppState;

pub fn dispatch_audio_feedback(
    feedback: &AudioFeedback,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    let mut result = DispatchResult::default();

    match feedback {
        AudioFeedback::PlayheadPosition(playhead) => {
            state.session.piano_roll.playhead = *playhead;
        }
        AudioFeedback::BpmUpdate(bpm) => {
            state.session.piano_roll.bpm = *bpm;
        }
        AudioFeedback::DrumSequencerStep { instrument_id, step } => {
            if let Some(inst) = state.instruments.instrument_mut(*instrument_id) {
                if let Some(seq) = inst.drum_sequencer.as_mut() {
                    seq.current_step = *step;
                    seq.last_played_step = Some(*step);
                }
            }
        }
        AudioFeedback::ServerStatus { status, message, server_running } => {
            result.push_status_with_running(*status, message.clone(), *server_running);
        }
        AudioFeedback::RecordingState { is_recording, elapsed_secs } => {
            state.recording = *is_recording;
            state.recording_secs = *elapsed_secs;
        }
        AudioFeedback::RecordingStopped(path) => {
            state.pending_recording_path = Some(path.clone());
        }
        AudioFeedback::CompileResult(res) => {
            match res {
                Ok(msg) => result.push_status(audio.status(), msg.clone()),
                Err(e) => result.push_status(audio.status(), e.clone()),
            }
        }
        AudioFeedback::PendingBufferFreed => {
            if let Some(path) = state.pending_recording_path.take() {
                let (peaks, _) = super::helpers::compute_waveform_peaks(&path.to_string_lossy());
                if !peaks.is_empty() {
                    state.recorded_waveform_peaks = Some(peaks);
                    result.push_nav(NavIntent::SwitchTo("waveform"));
                }
            }
        }
        AudioFeedback::VstParamsDiscovered { instrument_id, target, vst_plugin_id, params } => {
            // Update plugin registry with discovered param specs
            if let Some(plugin) = state.session.vst_plugins.get_mut(*vst_plugin_id) {
                plugin.params.clear();
                for (index, name, label, default) in params {
                    plugin.params.push(crate::state::VstParamSpec {
                        index: *index,
                        name: name.clone(),
                        default: *default,
                        label: label.clone(),
                    });
                }
            }
            // Initialize per-instance param values from defaults
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                match target {
                    VstTarget::Source => {
                        instrument.vst_param_values.clear();
                        for (index, _, _, default) in params {
                            instrument.vst_param_values.push((*index, *default));
                        }
                    }
                    VstTarget::Effect(idx) => {
                        if let Some(effect) = instrument.effects.get_mut(*idx) {
                            effect.vst_param_values.clear();
                            for (index, _, _, default) in params {
                                effect.vst_param_values.push((*index, *default));
                            }
                        }
                    }
                }
            }
        }
        AudioFeedback::VstStateSaved { instrument_id, target, path } => {
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                match target {
                    VstTarget::Source => {
                        instrument.vst_state_path = Some(path.clone());
                    }
                    VstTarget::Effect(idx) => {
                        if let Some(effect) = instrument.effects.get_mut(*idx) {
                            effect.vst_state_path = Some(path.clone());
                        }
                    }
                }
            }
        }
    }

    result
}
