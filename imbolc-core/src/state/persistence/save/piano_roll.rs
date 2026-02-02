use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;
use crate::state::instrument_state::InstrumentState;

pub(crate) fn save_piano_roll(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    // Tracks
    {
        let mut stmt = conn.prepare(
            "INSERT INTO piano_roll_tracks (instrument_id, position, polyphonic)
                 VALUES (?1, ?2, ?3)",
        )?;
        for (pos, &sid) in session.piano_roll.track_order.iter().enumerate() {
            if let Some(track) = session.piano_roll.tracks.get(&sid) {
                stmt.execute(rusqlite::params![sid, pos as i32, track.polyphonic])?;
            }
        }
    }

    // Notes
    {
        let mut stmt = conn.prepare(
            "INSERT INTO piano_roll_notes (track_instrument_id, tick, duration, pitch, velocity, probability)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for track in session.piano_roll.tracks.values() {
            for note in &track.notes {
                stmt.execute(rusqlite::params![
                    track.module_id,
                    note.tick,
                    note.duration,
                    note.pitch,
                    note.velocity,
                    note.probability as f64
                ])?;
            }
        }
    }

    // Musical settings
    conn.execute(
        "INSERT INTO musical_settings (id, bpm, time_sig_num, time_sig_denom, ticks_per_beat, loop_start, loop_end, looping, key, scale, tuning_a4, snap, swing_amount, humanize_velocity, humanize_timing)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            session.bpm as f64,
            session.time_signature.0,
            session.time_signature.1,
            session.piano_roll.ticks_per_beat,
            session.piano_roll.loop_start,
            session.piano_roll.loop_end,
            session.piano_roll.looping,
            session.key.name(),
            session.scale.name(),
            session.tuning_a4 as f64,
            session.snap,
            session.piano_roll.swing_amount as f64,
            session.humanize_velocity as f64,
            session.humanize_timing as f64,
        ],
    )?;
    Ok(())
}

pub(crate) fn save_sampler_configs(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut config_stmt = conn.prepare(
        "INSERT INTO sampler_configs (instrument_id, buffer_id, sample_name, loop_mode, pitch_tracking, next_slice_id, selected_slice)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut slice_stmt = conn.prepare(
        "INSERT INTO sampler_slices (instrument_id, slice_id, position, start_pos, end_pos, name, root_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for inst in &instruments.instruments {
        if let Some(ref config) = inst.sampler_config {
            config_stmt.execute(rusqlite::params![
                inst.id,
                config.buffer_id.map(|id| id as i32),
                config.sample_name,
                config.loop_mode,
                config.pitch_tracking,
                config.next_slice_id() as i32,
                config.selected_slice as i32,
            ])?;

            for (pos, slice) in config.slices.iter().enumerate() {
                slice_stmt.execute(rusqlite::params![
                    inst.id,
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
    Ok(())
}
