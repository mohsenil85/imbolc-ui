use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;
use crate::state::persistence::conversion::serialize_automation_target;
use crate::state::automation::CurveType;

pub(crate) fn save_automation(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let mut lane_stmt = conn.prepare(
        "INSERT INTO automation_lanes (id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, record_armed, min_value, max_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut point_stmt = conn.prepare(
        "INSERT INTO automation_points (lane_id, tick, value, curve_type)
             VALUES (?1, ?2, ?3, ?4)",
    )?;

    for lane in &session.automation.lanes {
        let (target_type, instrument_id, effect_idx, param_idx) =
            serialize_automation_target(&lane.target);

        lane_stmt.execute(rusqlite::params![
            lane.id as i32,
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
                point.tick as i32,
                point.value as f64,
                curve_str,
            ])?;
        }
    }
    Ok(())
}
