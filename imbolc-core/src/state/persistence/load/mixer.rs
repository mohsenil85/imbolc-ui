use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::session::MAX_BUSES;
use crate::state::instrument::MixerBus;

pub(crate) fn load_buses(conn: &SqlConnection) -> SqlResult<Vec<MixerBus>> {
    let mut buses: Vec<MixerBus> = (1..=MAX_BUSES as u8).map(MixerBus::new).collect();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, name, level, pan, mute, solo FROM mixer_buses ORDER BY id",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u8>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, bool>(5)?,
            ))
        }) {
            for result in rows {
                if let Ok((id, name, level, pan, mute, solo)) = result {
                    if let Some(bus) = buses.get_mut((id - 1) as usize) {
                        bus.name = name;
                        bus.level = level as f32;
                        bus.pan = pan as f32;
                        bus.mute = mute;
                        bus.solo = solo;
                    }
                }
            }
        }
    }
    Ok(buses)
}

pub(crate) fn load_master(conn: &SqlConnection) -> (f32, bool) {
    if let Ok(row) = conn.query_row(
        "SELECT level, mute FROM mixer_master WHERE id = 1",
        [],
        |row| Ok((row.get::<_, f64>(0)?, row.get::<_, bool>(1)?)),
    ) {
        (row.0 as f32, row.1)
    } else {
        (1.0, false)
    }
}
