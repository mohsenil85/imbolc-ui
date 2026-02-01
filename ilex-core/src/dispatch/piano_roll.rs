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
        }
        PianoRollAction::PlayStop => {
            let pr = &mut state.session.piano_roll;
            pr.playing = !pr.playing;
            if !pr.playing {
                pr.playhead = 0;
                if audio.is_running() {
                    audio.release_all_voices();
                }
                audio.clear_active_notes();
            }
            // Clear recording if stopping via normal play/stop
            state.session.piano_roll.recording = false;
        }
        PianoRollAction::PlayStopRecord => {
            let is_playing = state.session.piano_roll.playing;

            if !is_playing {
                // Start playing + recording
                state.session.piano_roll.playing = true;
                state.session.piano_roll.recording = true;
            } else {
                // Stop playing + recording
                let pr = &mut state.session.piano_roll;
                pr.playing = false;
                pr.playhead = 0;
                if audio.is_running() {
                    audio.release_all_voices();
                }
                audio.clear_active_notes();
                state.session.piano_roll.recording = false;
            }
        }
        PianoRollAction::ToggleLoop => {
            state.session.piano_roll.looping = !state.session.piano_roll.looping;
        }
        PianoRollAction::SetLoopStart(tick) => {
            state.session.piano_roll.loop_start = *tick;
        }
        PianoRollAction::SetLoopEnd(tick) => {
            state.session.piano_roll.loop_end = *tick;
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
        }
        PianoRollAction::TogglePolyMode(track_idx) => {
            if let Some(track) = state.session.piano_roll.track_at_mut(*track_idx) {
                track.polyphonic = !track.polyphonic;
            }
        }
        PianoRollAction::PlayNote { pitch, velocity, instrument_id, track } => {
            let pitch = *pitch;
            let velocity = *velocity;
            let instrument_id = *instrument_id;
            let track = *track;

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                let _ = audio.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                let duration_ticks = 240; // Half beat for staccato feel
                audio.push_active_note(instrument_id, pitch, duration_ticks);
            }

            // Record note if recording
            if state.session.piano_roll.recording {
                let playhead = state.session.piano_roll.playhead;
                let duration = 480; // One beat for live recording
                state.session.piano_roll.toggle_note(track, pitch, playhead, duration, velocity);
            }
        }
        PianoRollAction::PlayNotes { pitches, velocity, instrument_id, track } => {
            let velocity = *velocity;
            let instrument_id = *instrument_id;
            let track = *track;

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                for &pitch in pitches {
                    let _ = audio.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    audio.push_active_note(instrument_id, pitch, 240);
                }
            }

            // Record chord notes if recording
            if state.session.piano_roll.recording {
                let playhead = state.session.piano_roll.playhead;
                let duration = 480; // One beat for live recording
                for &pitch in pitches {
                    state.session.piano_roll.toggle_note(track, pitch, playhead, duration, velocity);
                }
            }
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
