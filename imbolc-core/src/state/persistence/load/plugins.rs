use std::path::PathBuf;
use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::{Instrument, InstrumentId};
use crate::state::persistence::conversion::deserialize_automation_target;

pub(crate) fn load_custom_synthdefs(conn: &SqlConnection) -> SqlResult<crate::state::custom_synthdef::CustomSynthDefRegistry> {
    use crate::state::custom_synthdef::{CustomSynthDef, CustomSynthDefRegistry, ParamSpec};

    let mut registry = CustomSynthDefRegistry::new();

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, name, synthdef_name, source_path FROM custom_synthdefs ORDER BY id",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
        }) {
            for result in rows {
                if let Ok((id, name, synthdef_name, source_path)) = result {
                    let synthdef = CustomSynthDef {
                        id, name, synthdef_name,
                        source_path: PathBuf::from(source_path),
                        params: Vec::new(),
                    };
                    registry.synthdefs.push(synthdef);
                    if id >= registry.next_id {
                        registry.next_id = id + 1;
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT synthdef_id, name, default_val, min_val, max_val FROM custom_synthdef_params ORDER BY synthdef_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?, row.get::<_, f64>(3)?, row.get::<_, f64>(4)?))
        }) {
            for result in rows {
                if let Ok((synthdef_id, name, default_val, min_val, max_val)) = result {
                    if let Some(synthdef) = registry.synthdefs.iter_mut().find(|s| s.id == synthdef_id) {
                        synthdef.params.push(ParamSpec {
                            name,
                            default: default_val as f32,
                            min: min_val as f32,
                            max: max_val as f32,
                        });
                    }
                }
            }
        }
    }

    Ok(registry)
}

pub(crate) fn load_vst_plugins(conn: &SqlConnection) -> SqlResult<crate::state::vst_plugin::VstPluginRegistry> {
    use crate::state::vst_plugin::{VstParamSpec, VstPlugin, VstPluginKind, VstPluginRegistry};

    let mut registry = VstPluginRegistry::new();

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, name, plugin_path, kind FROM vst_plugins ORDER BY id",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
        }) {
            for result in rows {
                if let Ok((id, name, plugin_path, kind_str)) = result {
                    let kind = match kind_str.as_str() {
                        "effect" => VstPluginKind::Effect,
                        _ => VstPluginKind::Instrument,
                    };
                    let plugin = VstPlugin {
                        id, name,
                        plugin_path: PathBuf::from(plugin_path),
                        kind,
                        params: Vec::new(),
                    };
                    registry.plugins.push(plugin);
                    if id >= registry.next_id {
                        registry.next_id = id + 1;
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT plugin_id, param_index, name, default_val FROM vst_plugin_params ORDER BY plugin_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?, row.get::<_, String>(2)?, row.get::<_, f64>(3)?))
        }) {
            for result in rows {
                if let Ok((plugin_id, param_index, name, default_val)) = result {
                    if let Some(plugin) = registry.plugins.iter_mut().find(|p| p.id == plugin_id) {
                        plugin.params.push(VstParamSpec {
                            index: param_index,
                            name,
                            default: default_val as f32,
                            label: None,
                        });
                    }
                }
            }
        }
    }

    Ok(registry)
}

pub(crate) fn load_vst_state_paths(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, vst_state_path FROM instruments WHERE vst_state_path IS NOT NULL",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, String>(1)?))
        }) {
            for result in rows {
                if let Ok((id, path_str)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == id) {
                        inst.vst_state_path = Some(PathBuf::from(path_str));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_vst_param_values(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, param_index, value FROM instrument_vst_params",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, f64>(2)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, param_index, value)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        inst.vst_param_values.push((param_index, value as f32));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_effect_vst_params(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, effect_position, param_index, value FROM effect_vst_params",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, usize>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, f64>(3)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, effect_pos, param_index, value)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(effect) = inst.effects.get_mut(effect_pos) {
                            effect.vst_param_values.push((param_index, value as f32));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_midi_recording(conn: &SqlConnection) -> SqlResult<crate::state::midi_recording::MidiRecordingState> {
    use crate::state::midi_recording::{MidiCcMapping, MidiRecordingState, PitchBendConfig, RecordMode};

    let mut state = MidiRecordingState::new();

    if let Ok(row) = conn.query_row(
        "SELECT live_input_instrument, note_passthrough, channel_filter
         FROM midi_recording_settings WHERE id = 1",
        [],
        |row| Ok((row.get::<_, Option<i32>>(0)?, row.get::<_, bool>(1)?, row.get::<_, Option<i32>>(2)?)),
    ) {
        state.live_input_instrument = row.0.map(|id| id as InstrumentId);
        state.note_passthrough = row.1;
        state.channel_filter = row.2.map(|c| c as u8);
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT cc_number, channel, target_type, target_instrument_id, target_effect_idx, target_param_idx, min_value, max_value
         FROM midi_cc_mappings",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?,
                row.get::<_, String>(2)?, row.get::<_, InstrumentId>(3)?,
                row.get::<_, Option<i32>>(4)?, row.get::<_, Option<i32>>(5)?,
                row.get::<_, f64>(6)?, row.get::<_, f64>(7)?,
            ))
        }) {
            for result in rows {
                if let Ok((cc_number, channel, target_type, instrument_id, effect_idx, param_idx, min_value, max_value)) = result {
                    if let Some(target) = deserialize_automation_target(&target_type, instrument_id, effect_idx, param_idx) {
                        let mut mapping = MidiCcMapping::new(cc_number as u8, target);
                        mapping.channel = channel.map(|c| c as u8);
                        mapping.min_value = min_value as f32;
                        mapping.max_value = max_value as f32;
                        state.cc_mappings.push(mapping);
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT target_type, target_instrument_id, target_effect_idx, target_param_idx, center_value, range, sensitivity
         FROM midi_pitch_bend_configs",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, row.get::<_, InstrumentId>(1)?,
                row.get::<_, Option<i32>>(2)?, row.get::<_, Option<i32>>(3)?,
                row.get::<_, f64>(4)?, row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
            ))
        }) {
            for result in rows {
                if let Ok((target_type, instrument_id, effect_idx, param_idx, center_value, range, sensitivity)) = result {
                    if let Some(target) = deserialize_automation_target(&target_type, instrument_id, effect_idx, param_idx) {
                        state.pitch_bend_configs.push(PitchBendConfig {
                            target,
                            center_value: center_value as f32,
                            range: range as f32,
                            sensitivity: sensitivity as f32,
                        });
                    }
                }
            }
        }
    }

    state.record_mode = RecordMode::Off;
    Ok(state)
}
