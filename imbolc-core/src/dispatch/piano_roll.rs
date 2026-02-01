use crate::audio::AudioHandle;
use crate::state::AppState;
use crate::action::{DispatchResult, PianoRollAction};

pub(super) fn dispatch_piano_roll(
    action: &PianoRollAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    match action {
        PianoRollAction::ToggleNote { pitch, tick, duration, velocity, track } => {
            state.session.piano_roll.toggle_note(*track, *pitch, *tick, *duration, *velocity);
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::PlayStop => {
            let pr = &mut state.session.piano_roll;
            pr.playing = !pr.playing;
            audio.set_playing(pr.playing);
            if !pr.playing {
                pr.playhead = 0;
                audio.reset_playhead();
                if audio.is_running() {
                    audio.release_all_voices();
                }
                audio.clear_active_notes();
            }
            // Clear recording if stopping via normal play/stop
            state.session.piano_roll.recording = false;
            return DispatchResult::none();
        }
        PianoRollAction::PlayStopRecord => {
            let is_playing = state.session.piano_roll.playing;

            if !is_playing {
                // Start playing + recording
                state.session.piano_roll.playing = true;
                audio.set_playing(true);
                state.session.piano_roll.recording = true;
            } else {
                // Stop playing + recording
                let pr = &mut state.session.piano_roll;
                pr.playing = false;
                pr.playhead = 0;
                audio.set_playing(false);
                audio.reset_playhead();
                if audio.is_running() {
                    audio.release_all_voices();
                }
                audio.clear_active_notes();
                state.session.piano_roll.recording = false;
            }
            return DispatchResult::none();
        }
        PianoRollAction::ToggleLoop => {
            state.session.piano_roll.looping = !state.session.piano_roll.looping;
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::SetLoopStart(tick) => {
            state.session.piano_roll.loop_start = *tick;
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::SetLoopEnd(tick) => {
            state.session.piano_roll.loop_end = *tick;
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::CycleTimeSig => {
            let pr = &mut state.session.piano_roll;
            pr.time_signature = match pr.time_signature {
                (4, 4) => (3, 4),
                (3, 4) => (6, 8),
                (6, 8) => (5, 4),
                (5, 4) => (7, 8),
                _ => (4, 4),
            };
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::TogglePolyMode(track_idx) => {
            if let Some(track) = state.session.piano_roll.track_at_mut(*track_idx) {
                track.polyphonic = !track.polyphonic;
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::PlayNote { pitch, velocity, instrument_id, track } => {
            let pitch = *pitch;
            let velocity = *velocity;
            let instrument_id = *instrument_id;
            let track = *track;

            // Expand to chord if instrument has a chord shape
            let chord_shape = state.instruments.instrument(instrument_id)
                .and_then(|inst| inst.chord_shape);
            let pitches: Vec<u8> = match chord_shape {
                Some(shape) => shape.expand(pitch),
                None => vec![pitch],
            };

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                for &p in &pitches {
                    let _ = audio.spawn_voice(instrument_id, p, vel_f, 0.0, &state.instruments, &state.session);
                    audio.push_active_note(instrument_id, p, 240);
                }
            }

            // Record note if recording
            if state.session.piano_roll.recording {
                let playhead = state.session.piano_roll.playhead;
                let duration = 480; // One beat for live recording
                for &p in &pitches {
                    state.session.piano_roll.toggle_note(track, p, playhead, duration, velocity);
                }
                let mut result = DispatchResult::none();
                result.audio_dirty.piano_roll = true;
                return result;
            }
            return DispatchResult::none();
        }
        PianoRollAction::PlayNotes { pitches, velocity, instrument_id, track } => {
            let velocity = *velocity;
            let instrument_id = *instrument_id;
            let track = *track;

            // Expand each pitch to chord if instrument has a chord shape
            let chord_shape = state.instruments.instrument(instrument_id)
                .and_then(|inst| inst.chord_shape);
            let all_pitches: Vec<u8> = pitches.iter().flat_map(|&pitch| {
                match chord_shape {
                    Some(shape) => shape.expand(pitch),
                    None => vec![pitch],
                }
            }).collect();

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                for &p in &all_pitches {
                    let _ = audio.spawn_voice(instrument_id, p, vel_f, 0.0, &state.instruments, &state.session);
                    audio.push_active_note(instrument_id, p, 240);
                }
            }

            // Record chord notes if recording
            if state.session.piano_roll.recording {
                let playhead = state.session.piano_roll.playhead;
                let duration = 480; // One beat for live recording
                for &p in &all_pitches {
                    state.session.piano_roll.toggle_note(track, p, playhead, duration, velocity);
                }
                let mut result = DispatchResult::none();
                result.audio_dirty.piano_roll = true;
                return result;
            }
            return DispatchResult::none();
        }
        PianoRollAction::AdjustSwing(delta) => {
            let pr = &mut state.session.piano_roll;
            pr.swing_amount = (pr.swing_amount + delta).clamp(0.0, 1.0);
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::MoveCursor(_, _)
        | PianoRollAction::SetBpm(_)
        | PianoRollAction::Zoom(_)
        | PianoRollAction::ScrollOctave(_) => {
            // Handled inside PianoRollPane — no state mutation needed
        }
    }
    DispatchResult::none()
}
