use std::path::Path;

use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use super::schema;
use super::load;
use super::save;
use crate::state::instrument::InstrumentId;
use crate::state::instrument_state::InstrumentState;
use crate::state::session::SessionState;

pub(crate) fn load_project_legacy(conn: &SqlConnection) -> SqlResult<(SessionState, InstrumentState)> {
    let has_layer_group_col = conn
        .prepare("SELECT next_layer_group_id FROM session LIMIT 0")
        .is_ok();
    let (next_id, selected_instrument, selected_automation_lane, next_layer_group_id): (InstrumentId, Option<i32>, Option<i32>, u32) =
        if has_layer_group_col {
            conn.query_row(
                "SELECT next_instrument_id, selected_instrument, selected_automation_lane, COALESCE(next_layer_group_id, 0) FROM session WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, i32>(3)? as u32)),
            )?
        } else {
            let (a, b, c): (InstrumentId, Option<i32>, Option<i32>) = conn.query_row(
                "SELECT next_instrument_id, selected_instrument, selected_automation_lane FROM session WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            (a, b, c, 0)
        };

    let mut instruments = load::load_instruments(conn)?;
    load::load_eq_bands(conn, &mut instruments)?;
    load::load_source_params(conn, &mut instruments)?;
    load::load_filter_params(conn, &mut instruments)?;
    load::load_effects(conn, &mut instruments)?;
    load::load_sends(conn, &mut instruments)?;
    load::load_modulations(conn, &mut instruments)?;
    load::load_sampler_configs(conn, &mut instruments)?;
    let buses = load::load_buses(conn)?;
    let (master_level, master_mute) = load::load_master(conn);
    let (piano_roll, musical) = load::load_piano_roll(conn)?;
    let mut automation = load::load_automation(conn)?;
    let custom_synthdefs = load::load_custom_synthdefs(conn)?;
    let vst_plugins = load::load_vst_plugins(conn)?;
    load::load_drum_sequencers(conn, &mut instruments)?;
    load::load_chopper_states(conn, &mut instruments)?;
    load::load_vst_state_paths(conn, &mut instruments)?;
    load::load_vst_param_values(conn, &mut instruments)?;
    load::load_effect_vst_params(conn, &mut instruments)?;
    load::load_arpeggiator_settings(conn, &mut instruments)?;
    load::load_layer_groups(conn, &mut instruments)?;
    let midi_recording = load::load_midi_recording(conn)?;
    let arrangement = load::load_arrangement(conn)?;

    // Restore selected_lane from DB, falling back to Some(0) if lanes exist
    automation.selected_lane = match selected_automation_lane {
        Some(idx) if (idx as usize) < automation.lanes.len() => Some(idx as usize),
        _ if !automation.lanes.is_empty() => Some(0),
        _ => None,
    };

    let mut session = SessionState::new();
    session.buses = buses;
    session.master_level = master_level;
    session.master_mute = master_mute;
    session.piano_roll = piano_roll;
    session.automation = automation;
    session.midi_recording = midi_recording;
    session.custom_synthdefs = custom_synthdefs;
    session.vst_plugins = vst_plugins;
    session.arrangement = arrangement;
    // Apply musical settings from load_piano_roll
    session.bpm = musical.bpm;
    session.time_signature = musical.time_signature;
    session.key = musical.key;
    session.scale = musical.scale;
    session.tuning_a4 = musical.tuning_a4;
    session.snap = musical.snap;
    session.humanize_velocity = musical.humanize_velocity;
    session.humanize_timing = musical.humanize_timing;

    let instrument_state = InstrumentState {
        instruments,
        selected: selected_instrument.map(|s| s as usize),
        next_id,
        next_sampler_buffer_id: 20000,
        editing_instrument_id: None,
        next_layer_group_id,
    };

    Ok((session, instrument_state))
}

#[allow(dead_code)]
pub(crate) fn save_project_legacy(path: &Path, session: &SessionState, instruments: &InstrumentState) -> SqlResult<()> {
    let conn = SqlConnection::open(path)?;

    schema::create_tables_and_clear(&conn)?;

    conn.execute(
        "INSERT INTO session (id, name, created_at, modified_at, next_instrument_id, selected_instrument, selected_automation_lane, next_layer_group_id)
             VALUES (1, 'default', datetime('now'), datetime('now'), ?1, ?2, ?3, ?4)",
        rusqlite::params![
            &instruments.next_id,
            instruments.selected.map(|s| s as i32),
            session.automation.selected_lane.map(|s| s as i32),
            &instruments.next_layer_group_id,
        ],
    )?;

    save::save_instruments(&conn, instruments)?;
    save::save_eq_bands(&conn, instruments)?;
    save::save_source_params(&conn, instruments)?;
    save::save_filter_params(&conn, instruments)?;
    save::save_effects(&conn, instruments)?;
    save::save_sends(&conn, instruments)?;
    save::save_modulations(&conn, instruments)?;
    save::save_mixer(&conn, session)?;
    save::save_piano_roll(&conn, session)?;
    save::save_sampler_configs(&conn, instruments)?;
    save::save_automation(&conn, session)?;
    save::save_custom_synthdefs(&conn, session)?;
    save::save_vst_plugins(&conn, session)?;
    save::save_drum_sequencers(&conn, instruments)?;
    save::save_chopper_states(&conn, instruments)?;
    save::save_midi_recording(&conn, session)?;
    save::save_vst_param_values(&conn, instruments)?;
    save::save_effect_vst_params(&conn, instruments)?;
    save::save_arrangement(&conn, session)?;

    Ok(())
}
