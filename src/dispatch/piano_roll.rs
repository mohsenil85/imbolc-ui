use crate::audio::AudioEngine;
use crate::panes::PianoRollPane;
use crate::state::AppState;
use crate::ui::{PaneManager, PianoRollAction};

pub(super) fn dispatch_piano_roll(
    action: &PianoRollAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
) {
    match action {
        PianoRollAction::ToggleNote => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let pitch = pr_pane.cursor_pitch();
                let tick = pr_pane.cursor_tick();
                let dur = pr_pane.default_duration();
                let vel = pr_pane.default_velocity();
                let track = pr_pane.current_track();
                state.session.piano_roll.toggle_note(track, pitch, tick, dur, vel);
            }
        }
        PianoRollAction::AdjustDuration(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_duration(delta);
            }
        }
        PianoRollAction::AdjustVelocity(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_velocity(delta);
            }
        }
        PianoRollAction::PlayStop => {
            let pr = &mut state.session.piano_roll;
            pr.playing = !pr.playing;
            if !pr.playing {
                pr.playhead = 0;
                if audio_engine.is_running() {
                    audio_engine.release_all_voices();
                }
                active_notes.clear();
            }
            // Clear recording if stopping via normal play/stop
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.set_recording(false);
            }
        }
        PianoRollAction::PlayStopRecord => {
            let is_playing = state.session.piano_roll.playing;

            if !is_playing {
                // Start playing + recording
                state.session.piano_roll.playing = true;
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.set_recording(true);
                }
            } else {
                // Stop playing + recording
                let pr = &mut state.session.piano_roll;
                pr.playing = false;
                pr.playhead = 0;
                if audio_engine.is_running() {
                    audio_engine.release_all_voices();
                }
                active_notes.clear();
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.set_recording(false);
                }
            }
        }
        PianoRollAction::ToggleLoop => {
            state.session.piano_roll.looping = !state.session.piano_roll.looping;
        }
        PianoRollAction::SetLoopStart => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                state.session.piano_roll.loop_start = tick;
            }
        }
        PianoRollAction::SetLoopEnd => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                state.session.piano_roll.loop_end = tick;
            }
        }
        PianoRollAction::ChangeTrack(delta) => {
            let delta = *delta;
            let track_count = state.session.piano_roll.track_order.len();
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.change_track(delta, track_count);
            }
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
        PianoRollAction::TogglePolyMode => {
            let track_idx = panes
                .get_pane_mut::<PianoRollPane>("piano_roll")
                .map(|pr| pr.current_track());
            if let Some(idx) = track_idx {
                if let Some(track) = state.session.piano_roll.track_at_mut(idx) {
                    track.polyphonic = !track.polyphonic;
                }
            }
        }
        PianoRollAction::Jump(_direction) => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.jump_to_end();
            }
        }
        PianoRollAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            // Get the current track's instrument_id
            let track_instrument_id: Option<u32> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    state.session.piano_roll.track_at(idx).map(|t| t.module_id)
                } else {
                    None
                }
            };

            if let Some(instrument_id) = track_instrument_id {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    let duration_ticks = 240; // Half beat for staccato feel
                    active_notes.push((instrument_id, pitch, duration_ticks));
                }

                // Record note if recording
                let recording_info = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .filter(|pr| pr.is_recording())
                    .map(|pr| (pr.current_track(), pr.default_duration(), pr.default_velocity()));
                if let Some((track_idx, duration, vel)) = recording_info {
                    let playhead = state.session.piano_roll.playhead;
                    state.session.piano_roll.toggle_note(track_idx, pitch, playhead, duration, vel);
                }
            }
        }
        PianoRollAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let track_instrument_id: Option<u32> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    state.session.piano_roll.track_at(idx).map(|t| t.module_id)
                } else {
                    None
                }
            };

            if let Some(instrument_id) = track_instrument_id {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                        active_notes.push((instrument_id, pitch, 240));
                    }
                }

                // Record chord notes if recording
                let recording_info = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .filter(|pr| pr.is_recording())
                    .map(|pr| (pr.current_track(), pr.default_duration(), pr.default_velocity()));
                if let Some((track_idx, duration, vel)) = recording_info {
                    let playhead = state.session.piano_roll.playhead;
                    for &pitch in pitches {
                        state.session.piano_roll.toggle_note(track_idx, pitch, playhead, duration, vel);
                    }
                }
            }
        }
        PianoRollAction::MoveCursor(_, _)
        | PianoRollAction::SetBpm(_)
        | PianoRollAction::Zoom(_)
        | PianoRollAction::ScrollOctave(_) => {
            // Reserved for future direct dispatch (currently handled inside PianoRollPane)
        }
    }
}
