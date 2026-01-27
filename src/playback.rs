use std::time::Duration;

use crate::audio::AudioEngine;
use crate::panes::RackPane;
use crate::ui::PaneManager;

/// Advance the piano roll playhead and process note-on/off events.
pub fn tick_playback(
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
    elapsed: Duration,
) {
    // Phase 1: advance playhead and collect note events (mutable borrow)
    let mut playback_data: Option<(
        Vec<(u32, u8, u8, u32, u32, bool)>, // note_ons: (module_id, pitch, vel, duration, tick, poly)
        u32,                                  // old_playhead
        u32,                                  // tick_delta
        f64,                                  // secs_per_tick
    )> = None;

    if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
        let pr = &mut rack_pane.rack_mut().piano_roll;
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

                let mut note_ons: Vec<(u32, u8, u8, u32, u32, bool)> = Vec::new();
                for &module_id in &pr.track_order {
                    if let Some(track) = pr.tracks.get(&module_id) {
                        let poly = track.polyphonic;
                        for note in &track.notes {
                            if note.tick >= scan_start && note.tick < scan_end {
                                note_ons.push((module_id, note.pitch, note.velocity, note.duration, note.tick, poly));
                            }
                        }
                    }
                }

                playback_data = Some((note_ons, old_playhead, tick_delta, secs_per_tick));
            }
        }
    }

    // Phase 2: send note-ons/offs (immutable rack borrow for poly chains)
    if let Some((note_ons, old_playhead, tick_delta, secs_per_tick)) = playback_data {
        if audio_engine.is_running() {
            let rack_clone = panes
                .get_pane_mut::<RackPane>("rack")
                .map(|r| r.rack().clone());

            if let Some(rack) = rack_clone {
                for &(module_id, pitch, velocity, duration, note_tick, polyphonic) in &note_ons {
                    let ticks_from_now = if note_tick >= old_playhead {
                        (note_tick - old_playhead) as f64
                    } else {
                        0.0
                    };
                    let offset = ticks_from_now * secs_per_tick;
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(module_id, pitch, vel_f, offset, polyphonic, &rack);
                    active_notes.push((module_id, pitch, duration));
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
            for (module_id, pitch, remaining) in &note_offs {
                let offset = *remaining as f64 * secs_per_tick;
                let _ = audio_engine.release_voice(*module_id, *pitch, offset);
            }
        }
    }
}
