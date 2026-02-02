use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::instrument_state::InstrumentState;

pub(crate) fn save_drum_sequencers(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut pad_stmt = conn.prepare(
        "INSERT INTO drum_pads (instrument_id, pad_index, buffer_id, path, name, level)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut pattern_stmt = conn.prepare(
        "INSERT INTO drum_patterns (instrument_id, pattern_index, length, swing_amount, chain_enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut chain_stmt = conn.prepare(
        "INSERT INTO drum_sequencer_chain (instrument_id, position, pattern_index) VALUES (?1, ?2, ?3)",
    )?;
    let mut step_stmt = conn.prepare(
        "INSERT INTO drum_steps (instrument_id, pattern_index, pad_index, step_index, velocity, probability)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for inst in &instruments.instruments {
        if let Some(seq) = &inst.drum_sequencer {
            let instrument_id = inst.id as i32;

            for (i, pad) in seq.pads.iter().enumerate() {
                pad_stmt.execute(rusqlite::params![
                    instrument_id,
                    i,
                    pad.buffer_id.map(|id| id as i32),
                    pad.path,
                    pad.name,
                    pad.level as f64,
                ])?;
            }

            for (pi, pattern) in seq.patterns.iter().enumerate() {
                pattern_stmt.execute(rusqlite::params![instrument_id, pi, pattern.length, seq.swing_amount as f64, seq.chain_enabled])?;

                for (pad_idx, pad_steps) in pattern.steps.iter().enumerate() {
                    for (step_idx, step) in pad_steps.iter().enumerate() {
                        if step.active {
                            step_stmt.execute(rusqlite::params![
                                instrument_id, pi, pad_idx, step_idx, step.velocity as i32, step.probability as f64
                            ])?;
                        }
                    }
                }
            }

            // Save pattern chain
            for (pos, &pattern_index) in seq.chain.iter().enumerate() {
                chain_stmt.execute(rusqlite::params![instrument_id, pos, pattern_index as i32])?;
            }
        }
    }
    Ok(())
}

pub(crate) fn save_chopper_states(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut header_stmt = conn.prepare(
        "INSERT INTO chopper_states (instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut slice_stmt = conn.prepare(
        "INSERT INTO chopper_slices (instrument_id, slice_id, position, start_pos, end_pos, name, root_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for inst in &instruments.instruments {
        if let Some(seq) = &inst.drum_sequencer {
            if let Some(chopper) = &seq.chopper {
                let instrument_id = inst.id as i32;

                header_stmt.execute(rusqlite::params![
                    instrument_id,
                    chopper.buffer_id.map(|id| id as i32),
                    chopper.path,
                    chopper.name,
                    chopper.selected_slice as i32,
                    chopper.next_slice_id as i32,
                    chopper.duration_secs as f64,
                ])?;

                for (pos, slice) in chopper.slices.iter().enumerate() {
                    slice_stmt.execute(rusqlite::params![
                        instrument_id,
                        slice.id as i32,
                        pos as i32,
                        slice.start as f64,
                        slice.end as f64,
                        &slice.name,
                        slice.root_note as i32,
                    ])?;
                }
            }
        }
    }
    Ok(())
}
