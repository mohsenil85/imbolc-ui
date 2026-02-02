use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::arrangement::{ArrangementState, Clip, ClipPlacement, PlayMode};
use crate::state::piano_roll::Note;

pub(crate) fn load_arrangement(conn: &SqlConnection) -> SqlResult<ArrangementState> {
    let mut arr = ArrangementState::new();

    // Check if tables exist (for backward compatibility with older DBs)
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='arrangement_clips'",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(arr);
    }

    // Load clips
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, instrument_id, length_ticks FROM arrangement_clips ORDER BY id",
        )?;
        let clips = stmt.query_map([], |row| {
            Ok(Clip {
                id: row.get(0)?,
                name: row.get(1)?,
                instrument_id: row.get(2)?,
                length_ticks: row.get(3)?,
                notes: Vec::new(),
            })
        })?;
        for clip in clips {
            arr.clips.push(clip?);
        }
    }

    // Load clip notes
    {
        let mut stmt = conn.prepare(
            "SELECT clip_id, tick, duration, pitch, velocity, probability FROM arrangement_clip_notes ORDER BY clip_id, tick",
        )?;
        let notes = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                Note {
                    tick: row.get(1)?,
                    duration: row.get(2)?,
                    pitch: row.get(3)?,
                    velocity: row.get(4)?,
                    probability: row.get::<_, f64>(5)? as f32,
                },
            ))
        })?;
        for result in notes {
            let (clip_id, note) = result?;
            if let Some(clip) = arr.clips.iter_mut().find(|c| c.id == clip_id) {
                clip.notes.push(note);
            }
        }
    }

    // Load placements
    {
        let mut stmt = conn.prepare(
            "SELECT id, clip_id, instrument_id, start_tick, length_override FROM arrangement_placements ORDER BY id",
        )?;
        let placements = stmt.query_map([], |row| {
            Ok(ClipPlacement {
                id: row.get(0)?,
                clip_id: row.get(1)?,
                instrument_id: row.get(2)?,
                start_tick: row.get(3)?,
                length_override: row.get::<_, Option<i32>>(4)?.map(|v| v as u32),
            })
        })?;
        for placement in placements {
            arr.placements.push(placement?);
        }
    }

    // Load settings
    {
        if let Ok(row) = conn.query_row(
            "SELECT play_mode, view_start_tick, ticks_per_col, cursor_tick, selected_lane, selected_placement
             FROM arrangement_settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, Option<i32>>(5)?,
                ))
            },
        ) {
            arr.play_mode = match row.0.as_str() {
                "song" => PlayMode::Song,
                _ => PlayMode::Pattern,
            };
            arr.view_start_tick = row.1;
            arr.ticks_per_col = row.2;
            arr.cursor_tick = row.3;
            arr.selected_lane = row.4 as usize;
            arr.selected_placement = row.5.map(|s| s as usize);
        }
    }

    arr.recalculate_next_ids();
    Ok(arr)
}
