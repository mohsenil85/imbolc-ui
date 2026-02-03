use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;
use crate::state::arrangement::PlayMode;
use crate::state::automation::CurveType;
use crate::state::persistence::conversion::serialize_automation_target;

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

    // Clip automation lanes
    {
        let mut lane_stmt = conn.prepare(
            "INSERT INTO arrangement_clip_automation_lanes (id, clip_id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, record_armed, min_value, max_value)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        let mut point_stmt = conn.prepare(
            "INSERT INTO arrangement_clip_automation_points (lane_id, clip_id, tick, value, curve_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for clip in &arr.clips {
            for lane in &clip.automation_lanes {
                let (target_type, instrument_id, effect_idx, param_idx) =
                    serialize_automation_target(&lane.target);
                lane_stmt.execute(rusqlite::params![
                    lane.id as i32,
                    clip.id,
                    target_type,
                    instrument_id,
                    effect_idx,
                    param_idx,
                    lane.enabled,
                    lane.record_armed,
                    lane.min_value as f64,
                    lane.max_value as f64,
                ])?;
                for point in &lane.points {
                    let curve_str = match point.curve {
                        CurveType::Linear => "linear",
                        CurveType::Exponential => "exponential",
                        CurveType::Step => "step",
                        CurveType::SCurve => "scurve",
                    };
                    point_stmt.execute(rusqlite::params![
                        lane.id as i32,
                        clip.id,
                        point.tick as i32,
                        point.value as f64,
                        curve_str,
                    ])?;
                }
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
