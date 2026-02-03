use crate::audio::AudioHandle;
use crate::state::AppState;
use crate::state::piano_roll::Note;
use crate::state::{ClipboardContents, ClipboardNote};
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
            // Ignore play/stop while exporting — user must cancel first
            if state.pending_export.is_some() || state.pending_render.is_some() {
                return DispatchResult::none();
            }
            let pr = &mut state.session.piano_roll;
            pr.playing = !pr.playing;
            audio.set_playing(pr.playing);
            if !pr.playing {
                state.audio_playhead = 0;
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
                state.audio_playhead = 0;
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

            // Fan-out to layer group members
            let targets = state.instruments.layer_group_members(instrument_id);

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                for &target_id in &targets {
                    if let Some(inst) = state.instruments.instrument(target_id) {
                        if state.effective_instrument_mute(inst) { continue; }
                        let expanded: Vec<u8> = match inst.chord_shape {
                            Some(shape) => shape.expand(pitch),
                            None => vec![pitch],
                        };
                        for &p in &expanded {
                            let _ = audio.spawn_voice(target_id, p, vel_f, 0.0, &state.instruments, &state.session);
                            audio.push_active_note(target_id, p, 240);
                        }
                    }
                }
            }

            // Record note only on the original track (not siblings)
            if state.session.piano_roll.recording {
                let chord_shape = state.instruments.instrument(instrument_id)
                    .and_then(|inst| inst.chord_shape);
                let record_pitches: Vec<u8> = match chord_shape {
                    Some(shape) => shape.expand(pitch),
                    None => vec![pitch],
                };
                let playhead = state.audio_playhead;
                let duration = 480; // One beat for live recording
                for &p in &record_pitches {
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

            // Fan-out to layer group members
            let targets = state.instruments.layer_group_members(instrument_id);

            if audio.is_running() {
                let vel_f = velocity as f32 / 127.0;
                for &target_id in &targets {
                    if let Some(inst) = state.instruments.instrument(target_id) {
                        if state.effective_instrument_mute(inst) { continue; }
                        for &pitch in pitches {
                            let expanded: Vec<u8> = match inst.chord_shape {
                                Some(shape) => shape.expand(pitch),
                                None => vec![pitch],
                            };
                            for &p in &expanded {
                                let _ = audio.spawn_voice(target_id, p, vel_f, 0.0, &state.instruments, &state.session);
                                audio.push_active_note(target_id, p, 240);
                            }
                        }
                    }
                }
            }

            // Record chord notes only on the original track (not siblings)
            if state.session.piano_roll.recording {
                let chord_shape = state.instruments.instrument(instrument_id)
                    .and_then(|inst| inst.chord_shape);
                let all_pitches: Vec<u8> = pitches.iter().flat_map(|&pitch| {
                    match chord_shape {
                        Some(shape) => shape.expand(pitch),
                        None => vec![pitch],
                    }
                }).collect();
                let playhead = state.audio_playhead;
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
        PianoRollAction::RenderToWav(instrument_id) => {
            let instrument_id = *instrument_id;
            if state.pending_render.is_some() || state.pending_export.is_some() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Running, "Already rendering or exporting");
            }
            if !audio.is_running() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Stopped, "Audio engine not running");
            }

            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let render_dir = std::path::Path::new(&home).join(".config/imbolc/renders");
            let _ = std::fs::create_dir_all(&render_dir);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let path = render_dir.join(format!("render_{}_{}.wav", instrument_id, timestamp));

            let pr = &mut state.session.piano_roll;
            state.pending_render = Some(crate::state::PendingRender {
                instrument_id,
                path: path.clone(),
                was_looping: pr.looping,
            });

            pr.playhead = pr.loop_start;
            pr.playing = true;
            pr.looping = false;

            if let Err(e) = audio.start_instrument_render(instrument_id, &path) {
                state.pending_render = None;
                state.session.piano_roll.playing = false;
                return DispatchResult::with_status(crate::audio::ServerStatus::Error, format!("Render failed: {}", e));
            }

            let mut result = DispatchResult::with_status(crate::audio::ServerStatus::Running, "Rendering...");
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::DeleteNotesInRegion { track, start_tick, end_tick, start_pitch, end_pitch } => {
            if let Some(t) = state.session.piano_roll.track_at_mut(*track) {
                t.notes.retain(|n| {
                    !(n.pitch >= *start_pitch && n.pitch <= *end_pitch
                      && n.tick >= *start_tick && n.tick < *end_tick)
                });
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::PasteNotes { track, anchor_tick, anchor_pitch, notes } => {
            if let Some(t) = state.session.piano_roll.track_at_mut(*track) {
                for cn in notes {
                    let tick = *anchor_tick + cn.tick_offset;
                    let pitch_i16 = *anchor_pitch as i16 + cn.pitch_offset;
                    if pitch_i16 < 0 || pitch_i16 > 127 { continue; }
                    let pitch = pitch_i16 as u8;
                    // Avoid duplicates at same (pitch, tick)
                    if !t.notes.iter().any(|n| n.pitch == pitch && n.tick == tick) {
                        let pos = t.notes.partition_point(|n| n.tick < tick);
                        t.notes.insert(pos, Note {
                            tick,
                            duration: cn.duration,
                            pitch,
                            velocity: cn.velocity,
                            probability: cn.probability,
                        });
                    }
                }
            }
            let mut result = DispatchResult::none();
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::BounceToWav => {
            if state.pending_render.is_some() || state.pending_export.is_some() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Running, "Already rendering or exporting");
            }
            if !audio.is_running() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Stopped, "Audio engine not running");
            }
            if state.instruments.instruments.is_empty() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Stopped, "No instruments");
            }

            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let export_dir = std::path::Path::new(&home).join(".config/imbolc/exports");
            let _ = std::fs::create_dir_all(&export_dir);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let path = export_dir.join(format!("bounce_{}.wav", timestamp));

            let pr = &mut state.session.piano_roll;
            state.pending_export = Some(crate::state::PendingExport {
                kind: crate::audio::commands::ExportKind::MasterBounce,
                was_looping: pr.looping,
                paths: vec![path.clone()],
            });

            pr.playhead = pr.loop_start;
            pr.playing = true;
            pr.looping = false;

            if let Err(e) = audio.start_master_bounce(&path) {
                state.pending_export = None;
                state.session.piano_roll.playing = false;
                return DispatchResult::with_status(
                    crate::audio::ServerStatus::Error,
                    format!("Bounce failed: {}", e),
                );
            }

            let mut result = DispatchResult::with_status(
                crate::audio::ServerStatus::Running,
                "Bouncing to WAV...",
            );
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::ExportStems => {
            if state.pending_render.is_some() || state.pending_export.is_some() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Running, "Already rendering or exporting");
            }
            if !audio.is_running() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Stopped, "Audio engine not running");
            }
            if state.instruments.instruments.is_empty() {
                return DispatchResult::with_status(crate::audio::ServerStatus::Stopped, "No instruments");
            }

            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let export_dir = std::path::Path::new(&home).join(".config/imbolc/exports");
            let _ = std::fs::create_dir_all(&export_dir);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut stems = Vec::new();
            let mut paths = Vec::new();
            for inst in &state.instruments.instruments {
                let safe_name: String = inst
                    .name
                    .replace(' ', "_")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect();
                let path = export_dir.join(format!("stem_{}_{}.wav", safe_name, timestamp));
                stems.push((inst.id, path.clone()));
                paths.push(path);
            }

            let pr = &mut state.session.piano_roll;
            state.pending_export = Some(crate::state::PendingExport {
                kind: crate::audio::commands::ExportKind::StemExport,
                was_looping: pr.looping,
                paths,
            });

            pr.playhead = pr.loop_start;
            pr.playing = true;
            pr.looping = false;

            if let Err(e) = audio.start_stem_export(&stems) {
                state.pending_export = None;
                state.session.piano_roll.playing = false;
                return DispatchResult::with_status(
                    crate::audio::ServerStatus::Error,
                    format!("Stem export failed: {}", e),
                );
            }

            let mut result = DispatchResult::with_status(
                crate::audio::ServerStatus::Running,
                format!("Exporting {} stems...", stems.len()),
            );
            result.audio_dirty.piano_roll = true;
            return result;
        }
        PianoRollAction::CancelExport => {
            if state.pending_export.is_some() {
                let _ = audio.cancel_export();
                let pr = &mut state.session.piano_roll;
                if let Some(export) = state.pending_export.take() {
                    pr.looping = export.was_looping;
                }
                pr.playing = false;
                state.audio_playhead = 0;
                state.export_progress = 0.0;
                audio.reset_playhead();
                let mut result = DispatchResult::with_status(
                    crate::audio::ServerStatus::Running,
                    "Export cancelled",
                );
                result.audio_dirty.piano_roll = true;
                return result;
            }
            return DispatchResult::none();
        }
        PianoRollAction::CopyNotes { track, start_tick, end_tick, start_pitch, end_pitch } => {
            if let Some(t) = state.session.piano_roll.track_at(*track) {
                let mut notes = Vec::new();
                for note in &t.notes {
                    if note.tick >= *start_tick && note.tick < *end_tick
                        && note.pitch >= *start_pitch && note.pitch <= *end_pitch
                    {
                        notes.push(ClipboardNote {
                            tick_offset: note.tick - start_tick,
                            pitch_offset: note.pitch as i16 - *start_pitch as i16,
                            duration: note.duration,
                            velocity: note.velocity,
                            probability: note.probability,
                        });
                    }
                }
                if !notes.is_empty() {
                    state.clipboard.contents = Some(ClipboardContents::PianoRollNotes(notes));
                }
            }
            return DispatchResult::none();
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
