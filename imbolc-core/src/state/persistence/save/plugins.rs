use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use crate::state::session::SessionState;
use crate::state::instrument_state::InstrumentState;
use crate::state::vst_plugin::VstPluginKind;
use crate::state::persistence::conversion::serialize_automation_target;

pub(crate) fn save_custom_synthdefs(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let mut synthdef_stmt = conn.prepare(
        "INSERT INTO custom_synthdefs (id, name, synthdef_name, source_path)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut param_stmt = conn.prepare(
        "INSERT INTO custom_synthdef_params (synthdef_id, position, name, default_val, min_val, max_val)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for synthdef in &session.custom_synthdefs.synthdefs {
        synthdef_stmt.execute(rusqlite::params![
            synthdef.id,
            &synthdef.name,
            &synthdef.synthdef_name,
            synthdef.source_path.to_string_lossy().as_ref(),
        ])?;

        for (pos, param) in synthdef.params.iter().enumerate() {
            param_stmt.execute(rusqlite::params![
                synthdef.id,
                pos as i32,
                &param.name,
                param.default as f64,
                param.min as f64,
                param.max as f64,
            ])?;
        }
    }

    Ok(())
}

pub(crate) fn save_vst_plugins(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let mut plugin_stmt = conn.prepare(
        "INSERT INTO vst_plugins (id, name, plugin_path, kind)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut param_stmt = conn.prepare(
        "INSERT INTO vst_plugin_params (plugin_id, position, param_index, name, default_val)
             VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for plugin in &session.vst_plugins.plugins {
        let kind_str = match plugin.kind {
            VstPluginKind::Instrument => "instrument",
            VstPluginKind::Effect => "effect",
        };
        plugin_stmt.execute(rusqlite::params![
            plugin.id,
            &plugin.name,
            plugin.plugin_path.to_string_lossy().as_ref(),
            kind_str,
        ])?;

        for (pos, param) in plugin.params.iter().enumerate() {
            param_stmt.execute(rusqlite::params![
                plugin.id,
                pos as i32,
                param.index as i32,
                &param.name,
                param.default as f64,
            ])?;
        }
    }

    Ok(())
}

pub(crate) fn save_vst_param_values(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instrument_vst_params (instrument_id, param_index, value)
             VALUES (?1, ?2, ?3)",
    )?;
    for inst in &instruments.instruments {
        for (idx, value) in &inst.vst_param_values {
            stmt.execute(rusqlite::params![
                inst.id,
                *idx as i32,
                *value as f64,
            ])?;
        }
    }
    Ok(())
}

pub(crate) fn save_effect_vst_params(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO effect_vst_params (instrument_id, effect_position, param_index, value)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    for inst in &instruments.instruments {
        for (pos, effect) in inst.effects.iter().enumerate() {
            for (idx, value) in &effect.vst_param_values {
                stmt.execute(rusqlite::params![
                    inst.id,
                    pos as i32,
                    *idx as i32,
                    *value as f64,
                ])?;
            }
        }
    }
    Ok(())
}

pub(crate) fn save_midi_recording(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let midi = &session.midi_recording;

    conn.execute(
        "INSERT INTO midi_recording_settings (id, live_input_instrument, note_passthrough, channel_filter)
             VALUES (1, ?1, ?2, ?3)",
        rusqlite::params![
            midi.live_input_instrument.map(|id| id as i32),
            midi.note_passthrough,
            midi.channel_filter.map(|c| c as i32),
        ],
    )?;

    let mut cc_stmt = conn.prepare(
        "INSERT INTO midi_cc_mappings (cc_number, channel, target_type, target_instrument_id, target_effect_idx, target_param_idx, min_value, max_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for mapping in &midi.cc_mappings {
        let (target_type, instrument_id, effect_idx, param_idx) =
            serialize_automation_target(&mapping.target);
        cc_stmt.execute(rusqlite::params![
            mapping.cc_number as i32,
            mapping.channel.map(|c| c as i32),
            target_type,
            instrument_id,
            effect_idx,
            param_idx,
            mapping.min_value as f64,
            mapping.max_value as f64,
        ])?;
    }

    let mut pb_stmt = conn.prepare(
        "INSERT INTO midi_pitch_bend_configs (target_type, target_instrument_id, target_effect_idx, target_param_idx, center_value, range, sensitivity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for config in &midi.pitch_bend_configs {
        let (target_type, instrument_id, effect_idx, param_idx) =
            serialize_automation_target(&config.target);
        pb_stmt.execute(rusqlite::params![
            target_type,
            instrument_id,
            effect_idx,
            param_idx,
            config.center_value as f64,
            config.range as f64,
            config.sensitivity as f64,
        ])?;
    }

    Ok(())
}
