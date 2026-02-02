use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::*;

pub(crate) fn load_drum_sequencers(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    use crate::state::drum_sequencer::DrumPattern;

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pad_index, buffer_id, path, name, level FROM drum_pads",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?,
                row.get::<_, Option<u32>>(2)?, row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?, row.get::<_, f64>(5)?,
            ))
        }) {
            for row in rows {
                if let Ok((instrument_id, idx, buffer_id, path, name, level)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pad) = seq.pads.get_mut(idx) {
                                pad.buffer_id = buffer_id;
                                pad.path = path;
                                pad.name = name;
                                pad.level = level as f32;
                            }
                        }
                    }
                }
            }
        }
    }

    for inst in instruments.iter_mut() {
        if let Some(seq) = &mut inst.drum_sequencer {
            let max_id = seq.pads.iter().filter_map(|p| p.buffer_id).max().unwrap_or(9999);
            seq.next_buffer_id = max_id + 1;
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pattern_index, length FROM drum_patterns ORDER BY instrument_id, pattern_index",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?, row.get::<_, usize>(2)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, idx, length)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pattern) = seq.patterns.get_mut(idx) {
                                *pattern = DrumPattern::new(length);
                            }
                        }
                    }
                }
            }
        }
    }

    // Load swing_amount (optional, may not exist in older schemas)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, swing_amount FROM drum_patterns WHERE pattern_index = 0",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, f64>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, swing)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.swing_amount = swing as f32;
                        }
                    }
                }
            }
        }
    }

    let has_step_probability = conn
        .prepare("SELECT probability FROM drum_steps LIMIT 0")
        .is_ok();
    let steps_query = if has_step_probability {
        "SELECT instrument_id, pattern_index, pad_index, step_index, velocity, probability FROM drum_steps"
    } else {
        "SELECT instrument_id, pattern_index, pad_index, step_index, velocity FROM drum_steps"
    };
    if let Ok(mut stmt) = conn.prepare(steps_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let probability = if has_step_probability { row.get::<_, f32>(5).unwrap_or(1.0) } else { 1.0 };
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?,
                row.get::<_, usize>(2)?, row.get::<_, usize>(3)?,
                row.get::<_, u8>(4)?, probability,
            ))
        }) {
            for row in rows {
                if let Ok((instrument_id, pi, pad_idx, step_idx, velocity, probability)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pattern) = seq.patterns.get_mut(pi) {
                                if let Some(step) = pattern.steps.get_mut(pad_idx).and_then(|s| s.get_mut(step_idx)) {
                                    step.active = true;
                                    step.velocity = velocity;
                                    step.probability = probability;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Load chain_enabled (optional, may not exist in older schemas)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, chain_enabled FROM drum_patterns WHERE pattern_index = 0",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, bool>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, chain_enabled)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chain_enabled = chain_enabled;
                        }
                    }
                }
            }
        }
    }

    // Load pattern chain
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pattern_index FROM drum_sequencer_chain ORDER BY instrument_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, pattern_index)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chain.push(pattern_index);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn load_chopper_states(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    use crate::state::drum_sequencer::ChopperState;
    use crate::state::sampler::Slice;

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs
         FROM chopper_states",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, Option<i32>>(1)?,
                row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?,
                row.get::<_, usize>(4)?, row.get::<_, u32>(5)?,
                row.get::<_, f64>(6)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chopper = Some(ChopperState {
                                buffer_id: buffer_id.map(|id| id as u32),
                                path, name,
                                slices: Vec::new(),
                                selected_slice, next_slice_id,
                                waveform_peaks: Vec::new(),
                                duration_secs: duration_secs as f32,
                            });
                        }
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, slice_id, start_pos, end_pos, name, root_note
         FROM chopper_slices ORDER BY instrument_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?, row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?, row.get::<_, i32>(5)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, slice_id, start, end, name, root_note)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(chopper) = &mut seq.chopper {
                                chopper.slices.push(Slice {
                                    id: slice_id as u32,
                                    start: start as f32,
                                    end: end as f32,
                                    name,
                                    root_note: root_note as u8,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
