use std::time::Duration;

use crate::audio::AudioEngine;
use crate::state::{AppState, StripState};

/// Advance the piano roll playhead and process note-on/off events.
pub fn tick_playback(
    state: &mut AppState,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
    elapsed: Duration,
) {
    // Phase 1: advance playhead and collect note events
    let mut playback_data: Option<(
        Vec<(u32, u8, u8, u32, u32)>, // note_ons: (strip_id, pitch, vel, duration, tick)
        u32,                           // old_playhead
        u32,                           // new_playhead
        u32,                           // tick_delta
        f64,                           // secs_per_tick
    )> = None;

    {
        let pr = &mut state.strip.piano_roll;
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
                for &strip_id in &pr.track_order {
                    if let Some(track) = pr.tracks.get(&strip_id) {
                        for note in &track.notes {
                            if note.tick >= scan_start && note.tick < scan_end {
                                note_ons.push((strip_id, note.pitch, note.velocity, note.duration, note.tick));
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
            for &(strip_id, pitch, velocity, duration, note_tick) in &note_ons {
                let ticks_from_now = if note_tick >= old_playhead {
                    (note_tick - old_playhead) as f64
                } else {
                    0.0
                };
                let offset = ticks_from_now * secs_per_tick;
                let vel_f = velocity as f32 / 127.0;
                let _ = audio_engine.spawn_voice(strip_id, pitch, vel_f, offset, &state.strip);
                active_notes.push((strip_id, pitch, duration));
            }

            // Process automation
            process_automation(audio_engine, &state.strip, new_playhead);
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
            for (strip_id, pitch, remaining) in &note_offs {
                let offset = *remaining as f64 * secs_per_tick;
                let _ = audio_engine.release_voice(*strip_id, *pitch, offset, &state.strip);
            }
        }
    }
}

/// Process automation lanes and apply values at the current playhead
fn process_automation(
    audio_engine: &AudioEngine,
    state: &StripState,
    playhead: u32,
) {
    for lane in &state.automation.lanes {
        if !lane.enabled {
            continue;
        }
        if let Some(value) = lane.value_at(playhead) {
            let _ = audio_engine.apply_automation(&lane.target, value, state);
        }
    }
}
