use std::sync::mpsc::Sender;
use std::time::Duration;

use super::commands::AudioFeedback;
use super::engine::AudioEngine;
use super::snapshot::{InstrumentSnapshot, SessionSnapshot};

pub fn tick_drum_sequencer(
    instruments: &mut InstrumentSnapshot,
    session: &SessionSnapshot,
    bpm: f32,
    engine: &mut AudioEngine,
    rng_state: &mut u64,
    feedback_tx: &Sender<AudioFeedback>,
    elapsed: Duration,
) {
    for instrument in &mut instruments.instruments {
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

        // Swing: odd-numbered steps need a higher threshold to fire (delayed)
        let next_step = (seq.current_step + 1) % pattern_length;
        let swing_threshold = if seq.swing_amount > 0.0 && next_step % 2 == 1 {
            1.0 + seq.swing_amount * 0.5
        } else if seq.swing_amount > 0.0 && seq.current_step % 2 == 1 {
            // After a swung step, the following even step comes sooner
            1.0 - seq.swing_amount * 0.5
        } else {
            1.0
        };

        while seq.step_accumulator >= swing_threshold {
            seq.step_accumulator -= swing_threshold;
            let next = seq.current_step + 1;
            if next >= pattern_length {
                // Pattern wrapped — advance chain if enabled
                if seq.chain_enabled && !seq.chain.is_empty() {
                    seq.chain_position = (seq.chain_position + 1) % seq.chain.len();
                    let next_pattern = seq.chain[seq.chain_position];
                    if next_pattern < seq.patterns.len() {
                        seq.current_pattern = next_pattern;
                    }
                }
                seq.current_step = 0;
            } else {
                seq.current_step = next;
            }
        }

        if seq.last_played_step != Some(seq.current_step) {
            if engine.is_running() && !instrument.mute {
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
                                // Probability check: skip hit if random exceeds probability
                                if step.probability < 1.0 {
                                    *rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                                    let r = ((*rng_state >> 33) as f32) / (u32::MAX as f32);
                                    if r > step.probability { continue; }
                                }
                                let mut amp = (step.velocity as f32 / 127.0) * pad.level;
                                // Velocity humanization
                                if session.humanize_velocity > 0.0 {
                                    *rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                                    let r = ((*rng_state >> 33) as f32) / (u32::MAX as f32);
                                    let jitter = (r - 0.5) * 2.0 * session.humanize_velocity * (30.0 / 127.0);
                                    amp = (amp + jitter).clamp(0.01, 1.0);
                                }
                                let total_pitch = pad.pitch as i16 + step.pitch_offset as i16;
                                let pitch_rate = 2.0_f32.powf(total_pitch as f32 / 12.0);
                                let rate = if pad.reverse { -pitch_rate } else { pitch_rate };
                                let _ = engine.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument.id,
                                    pad.slice_start, pad.slice_end, rate,
                                );
                            }
                        }
                    }
                }
            }
            let _ = feedback_tx.send(AudioFeedback::DrumSequencerStep {
                instrument_id: instrument.id,
                step: seq.current_step,
            });
            seq.last_played_step = Some(seq.current_step);
        }
    }
}
