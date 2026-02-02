use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::InstrumentId;
use crate::state::persistence::conversion::deserialize_automation_target;

pub(crate) fn load_automation(conn: &SqlConnection) -> SqlResult<crate::state::automation::AutomationState> {
    use crate::state::automation::{AutomationLane, AutomationPoint, AutomationState, CurveType};

    let mut state = AutomationState::new();

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, min_value, max_value, record_armed
         FROM automation_lanes",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?, row.get::<_, String>(1)?,
                row.get::<_, InstrumentId>(2)?, row.get::<_, Option<i32>>(3)?,
                row.get::<_, Option<i32>>(4)?, row.get::<_, bool>(5)?,
                row.get::<_, f64>(6)?, row.get::<_, f64>(7)?,
                row.get::<_, Option<bool>>(8).unwrap_or(None),
            ))
        }) {
            for result in rows {
                if let Ok((id, target_type, instrument_id, effect_idx, param_idx, enabled, min_value, max_value, record_armed)) = result {
                    if let Some(target) = deserialize_automation_target(&target_type, instrument_id, effect_idx, param_idx) {
                        let mut lane = AutomationLane::new(id as u32, target);
                        lane.enabled = enabled;
                        lane.record_armed = record_armed.unwrap_or(false);
                        lane.min_value = min_value as f32;
                        lane.max_value = max_value as f32;
                        state.lanes.push(lane);
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT lane_id, tick, value, curve_type FROM automation_points ORDER BY lane_id, tick",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?, row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?, row.get::<_, String>(3)?,
            ))
        }) {
            for result in rows {
                if let Ok((lane_id, tick, value, curve_type)) = result {
                    let curve = match curve_type.as_str() {
                        "linear" => CurveType::Linear,
                        "exponential" => CurveType::Exponential,
                        "step" => CurveType::Step,
                        "scurve" => CurveType::SCurve,
                        _ => CurveType::Linear,
                    };
                    if let Some(lane) = state.lanes.iter_mut().find(|l| l.id == lane_id as u32) {
                        lane.points.push(AutomationPoint::with_curve(tick as u32, value as f32, curve));
                    }
                }
            }
        }
    }

    state.recalculate_next_lane_id();
    Ok(state)
}
