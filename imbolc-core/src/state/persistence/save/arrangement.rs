use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;
use crate::state::arrangement::PlayMode;

pub(crate) fn save_arrangement(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let arr = &session.arrangement;

    // Clips
    {
        let mut stmt = conn.prepare(
            "INSERT INTO arrangement_clips (id, name, instrument_id, length_ticks)
                 VALUES (?1, ?2, ?3, ?4)",
        )?;
        for clip in &arr.clips {
            stmt.execute(rusqlite::params![
                clip.id,
                &clip.name,
                clip.instrument_id,
                clip.length_ticks,
            ])?;
        }
    }

    // Clip notes
    {
        let mut stmt = conn.prepare(
            "INSERT INTO arrangement_clip_notes (clip_id, tick, duration, pitch, velocity, probability)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for clip in &arr.clips {
            for note in &clip.notes {
                stmt.execute(rusqlite::params![
                    clip.id,
                    note.tick,
                    note.duration,
                    note.pitch,
                    note.velocity,
                    note.probability as f64,
                ])?;
            }
        }
    }

    // Placements
    {
        let mut stmt = conn.prepare(
            "INSERT INTO arrangement_placements (id, clip_id, instrument_id, start_tick, length_override)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for placement in &arr.placements {
            stmt.execute(rusqlite::params![
                placement.id,
                placement.clip_id,
                placement.instrument_id,
                placement.start_tick,
                placement.length_override.map(|l| l as i32),
            ])?;
        }
    }

    // Settings
    {
        let mode_str = match arr.play_mode {
            PlayMode::Pattern => "pattern",
            PlayMode::Song => "song",
        };
        conn.execute(
            "INSERT INTO arrangement_settings (id, play_mode, view_start_tick, ticks_per_col, cursor_tick, selected_lane, selected_placement)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                mode_str,
                arr.view_start_tick,
                arr.ticks_per_col,
                arr.cursor_tick,
                arr.selected_lane as i32,
                arr.selected_placement.map(|s| s as i32),
            ],
        )?;
    }

    Ok(())
}
