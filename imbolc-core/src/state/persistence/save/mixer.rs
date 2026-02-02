use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;

pub(crate) fn save_mixer(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO mixer_buses (id, name, level, pan, mute, solo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for bus in &session.buses {
        stmt.execute(rusqlite::params![
            bus.id,
            bus.name,
            bus.level as f64,
            bus.pan as f64,
            bus.mute,
            bus.solo
        ])?;
    }

    conn.execute(
        "INSERT INTO mixer_master (id, level, mute) VALUES (1, ?1, ?2)",
        rusqlite::params![session.master_level as f64, session.master_mute],
    )?;
    Ok(())
}
