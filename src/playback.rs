use std::time::Duration;

use crate::audio::AudioEngine;
use crate::state::AppState;

/// Advance the piano roll playhead and process note-on/off events.
pub fn tick_playback(
    state: &mut AppState,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
    elapsed: Duration,
) {
    // Phase 1: advance playhead and collect note events
    let mut playback_data: Option<(
        Vec<(u32, u8, u8, u32, u32)>, // note_ons: (instrument_id, pitch, vel, duration, tick)
        u32,                           // old_playhead
        u32,                           // new_playhead
        u32,                           // tick_delta
        f64,                           // secs_per_tick
    )> = None;

    {
        let pr = &mut state.session.piano_roll;
        if pr.playing {
            let seconds = elapsed.as_secs_f32();
            let ticks_f = seconds * (pr.bpm / 60.0) * pr.ticks_per_beat as f32;
            let tick_delta = ticks_f as u32;

            if tick_delta > 0 {
                let old_playhead = pr.playhead;
                pr.advance(tick_delta);
                let new_playhead = pr.playhead;

                let (scan_start, scan_end) = if new_playhead >= old_playhead {
                    (old_playhead, new_playhead)
                } else {
                    (pr.loop_start, new_playhead)
                };

                let secs_per_tick = 60.0 / (pr.bpm as f64 * pr.ticks_per_beat as f64);

                let mut note_ons: Vec<(u32, u8, u8, u32, u32)> = Vec::new();
                for &instrument_id in &pr.track_order {
                    if let Some(track) = pr.tracks.get(&instrument_id) {
                        for note in &track.notes {
                            if note.tick >= scan_start && note.tick < scan_end {
                                note_ons.push((instrument_id, note.pitch, note.velocity, note.duration, note.tick));
                            }
                        }
                    }
                }

                playback_data = Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick));
            }
        }
    }

    // Phase 2: send note-ons/offs and process automation (shared borrow only)
    if let Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick)) = playback_data {
        if audio_engine.is_running() {
            // Process note-ons
            for &(instrument_id, pitch, velocity, duration, note_tick) in &note_ons {
                let ticks_from_now = if note_tick >= old_playhead {
                    (note_tick - old_playhead) as f64
                } else {
                    0.0
                };
                let offset = ticks_from_now * secs_per_tick;
                let vel_f = velocity as f32 / 127.0;
                let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, offset, &state.instruments, &state.session);
                active_notes.push((instrument_id, pitch, duration));
            }

            // Process automation
            for lane in &state.session.automation.lanes {
                if !lane.enabled {
                    continue;
                }
                if let Some(value) = lane.value_at(new_playhead) {
                    let _ = audio_engine.apply_automation(&lane.target, value, &state.instruments, &state.session);
                }
            }
        }

        // Process active notes: decrement remaining ticks, send note-offs
        let mut note_offs: Vec<(u32, u8, u32)> = Vec::new();
        for note in active_notes.iter_mut() {
            if note.2 <= tick_delta {
                note_offs.push((note.0, note.1, note.2));
                note.2 = 0;
            } else {
                note.2 -= tick_delta;
            }
        }
        active_notes.retain(|n| n.2 > 0);

        if audio_engine.is_running() {
            for (instrument_id, pitch, remaining) in &note_offs {
                let offset = *remaining as f64 * secs_per_tick;
                let _ = audio_engine.release_voice(*instrument_id, *pitch, offset, &state.instruments);
            }
        }
    }
}

/// Advance the drum sequencer for each drum machine instrument and trigger pad hits.
pub fn tick_drum_sequencer(
    state: &mut AppState,
    audio_engine: &mut AudioEngine,
    elapsed: Duration,
) {
    let bpm = state.session.piano_roll.bpm;

    for instrument in &mut state.instruments.instruments {
        let seq = match &mut instrument.drum_sequencer {
            Some(s) => s,
            None => continue,
        };
        if !seq.playing {
            seq.last_played_step = None;
            continue;
        }

        let pattern_length = seq.pattern().length;
        let steps_per_beat = 4.0_f32;
        let steps_per_second = (bpm / 60.0) * steps_per_beat;

        seq.step_accumulator += elapsed.as_secs_f32() * steps_per_second;

        while seq.step_accumulator >= 1.0 {
            seq.step_accumulator -= 1.0;
            seq.current_step = (seq.current_step + 1) % pattern_length;
        }

        if seq.last_played_step != Some(seq.current_step) {
            if audio_engine.is_running() && !instrument.mute {
                let current_step = seq.current_step;
                let current_pattern = seq.current_pattern;
                let pattern = &seq.patterns[current_pattern];
                for (pad_idx, pad) in seq.pads.iter().enumerate() {
                    if let Some(buffer_id) = pad.buffer_id {
                        if let Some(step) = pattern
                            .steps
                            .get(pad_idx)
                            .and_then(|s| s.get(current_step))
                        {
                            if step.active {
                                let amp = (step.velocity as f32 / 127.0) * pad.level;
                                let _ = audio_engine.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument.id,
                                );
                            }
                        }
                    }
                }
            }
            seq.last_played_step = Some(seq.current_step);
        }
    }
}
