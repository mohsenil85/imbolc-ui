use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::arrangement::{ArrangementState, Clip, ClipPlacement, PlayMode};
use crate::state::automation::{AutomationLane, AutomationPoint, CurveType};
use crate::state::instrument::InstrumentId;
use crate::state::persistence::conversion::deserialize_automation_target;
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
                automation_lanes: Vec::new(),
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

    // Load clip automation lanes (backward-compatible: table may not exist)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, clip_id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, record_armed, min_value, max_value
         FROM arrangement_clip_automation_lanes",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, InstrumentId>(3)?,
                row.get::<_, Option<i32>>(4)?,
                row.get::<_, Option<i32>>(5)?,
                row.get::<_, bool>(6)?,
                row.get::<_, Option<bool>>(7).unwrap_or(None),
                row.get::<_, f64>(8)?,
                row.get::<_, f64>(9)?,
            ))
        }) {
            for result in rows {
                if let Ok((id, clip_id, target_type, instrument_id, effect_idx, param_idx, enabled, record_armed, min_value, max_value)) = result {
                    if let Some(target) = deserialize_automation_target(&target_type, instrument_id, effect_idx, param_idx) {
                        let mut lane = AutomationLane::new(id as u32, target);
                        lane.enabled = enabled;
                        lane.record_armed = record_armed.unwrap_or(false);
                        lane.min_value = min_value as f32;
                        lane.max_value = max_value as f32;
                        if let Some(clip) = arr.clips.iter_mut().find(|c| c.id == clip_id) {
                            clip.automation_lanes.push(lane);
                        }
                    }
                }
            }
        }
    }

    // Load clip automation points (backward-compatible: table may not exist)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT lane_id, clip_id, tick, value, curve_type FROM arrangement_clip_automation_points ORDER BY lane_id, tick",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        }) {
            for result in rows {
                if let Ok((lane_id, clip_id, tick, value, curve_type)) = result {
                    let curve = match curve_type.as_str() {
                        "linear" => CurveType::Linear,
                        "exponential" => CurveType::Exponential,
                        "step" => CurveType::Step,
                        "scurve" => CurveType::SCurve,
                        _ => CurveType::Linear,
                    };
                    if let Some(clip) = arr.clips.iter_mut().find(|c| c.id == clip_id) {
                        if let Some(lane) = clip.automation_lanes.iter_mut().find(|l| l.id == lane_id as u32) {
                            lane.points.push(AutomationPoint::with_curve(tick as u32, value as f32, curve));
                        }
                    }
                }
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
