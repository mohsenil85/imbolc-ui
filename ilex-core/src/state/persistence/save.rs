use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use super::super::instrument::*;
use super::super::instrument_state::InstrumentState;
use super::super::param::ParamValue;
use super::super::session::SessionState;
use super::super::vst_plugin::VstPluginKind;
use super::conversion::serialize_automation_target;

pub(super) fn save_instruments(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instruments (id, name, position, source_type, filter_type, filter_cutoff, filter_resonance,
             lfo_enabled, lfo_rate, lfo_depth, lfo_shape, lfo_target,
             amp_attack, amp_decay, amp_sustain, amp_release, polyphonic,
             level, pan, mute, solo, active, output_target)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
    )?;
    for (pos, inst) in instruments.instruments.iter().enumerate() {
        let source_str = match inst.source {
            SourceType::Custom(id) => format!("custom:{}", id),
            SourceType::Vst(id) => format!("vst:{}", id),
            _ => inst.source.short_name().to_string(),
        };
        let (filter_type, filter_cutoff, filter_res): (Option<String>, Option<f64>, Option<f64>) =
            if let Some(ref f) = inst.filter {
                (
                    Some(format!("{:?}", f.filter_type).to_lowercase()),
                    Some(f.cutoff.value as f64),
                    Some(f.resonance.value as f64),
                )
            } else {
                (None, None, None)
            };
        let lfo_shape_str = match inst.lfo.shape {
            LfoShape::Sine => "sine",
            LfoShape::Square => "square",
            LfoShape::Saw => "saw",
            LfoShape::Triangle => "triangle",
        };
        let lfo_target_str = match inst.lfo.target {
            LfoTarget::FilterCutoff => "filter_cutoff",
            LfoTarget::FilterResonance => "filter_res",
            LfoTarget::Amplitude => "amp",
            LfoTarget::Pitch => "pitch",
            LfoTarget::Pan => "pan",
            LfoTarget::PulseWidth => "pulse_width",
            LfoTarget::SampleRate => "sample_rate",
            LfoTarget::DelayTime => "delay_time",
            LfoTarget::DelayFeedback => "delay_feedback",
            LfoTarget::ReverbMix => "reverb_mix",
            LfoTarget::GateRate => "gate_rate",
            LfoTarget::SendLevel => "send_level",
            LfoTarget::Detune => "detune",
            LfoTarget::Attack => "attack",
            LfoTarget::Release => "release",
        };
        let output_str = match inst.output_target {
            OutputTarget::Master => "master".to_string(),
            OutputTarget::Bus(n) => format!("bus:{}", n),
        };
        stmt.execute(rusqlite::params![
            inst.id,
            inst.name,
            pos as i32,
            source_str,
            filter_type,
            filter_cutoff,
            filter_res,
            inst.lfo.enabled,
            inst.lfo.rate as f64,
            inst.lfo.depth as f64,
            lfo_shape_str,
            lfo_target_str,
            inst.amp_envelope.attack as f64,
            inst.amp_envelope.decay as f64,
            inst.amp_envelope.sustain as f64,
            inst.amp_envelope.release as f64,
            inst.polyphonic,
            inst.level as f64,
            inst.pan as f64,
            inst.mute,
            inst.solo,
            inst.active,
            output_str,
        ])?;
    }
    Ok(())
}

pub(super) fn save_source_params(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instrument_source_params (instrument_id, param_name, param_value, param_min, param_max, param_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for inst in &instruments.instruments {
        for param in &inst.source_params {
            let (value, param_type) = match &param.value {
                ParamValue::Float(v) => (*v as f64, "float"),
                ParamValue::Int(v) => (*v as f64, "int"),
                ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, "bool"),
            };
            stmt.execute(rusqlite::params![
                inst.id,
                param.name,
                value,
                param.min as f64,
                param.max as f64,
                param_type,
            ])?;
        }
    }
    Ok(())
}

pub(super) fn save_effects(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut effect_stmt = conn.prepare(
        "INSERT INTO instrument_effects (instrument_id, position, effect_type, enabled)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut param_stmt = conn.prepare(
        "INSERT INTO instrument_effect_params (instrument_id, effect_position, param_name, param_value)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    for inst in &instruments.instruments {
        for (pos, effect) in inst.effects.iter().enumerate() {
            let type_str = match effect.effect_type {
                EffectType::Vst(id) => format!("vst:{}", id),
                _ => format!("{:?}", effect.effect_type).to_lowercase(),
            };
            effect_stmt.execute(rusqlite::params![
                inst.id,
                pos as i32,
                type_str,
                effect.enabled
            ])?;
            for param in &effect.params {
                let value = match &param.value {
                    ParamValue::Float(v) => *v as f64,
                    ParamValue::Int(v) => *v as f64,
                    ParamValue::Bool(v) => {
                        if *v { 1.0 } else { 0.0 }
                    }
                };
                param_stmt.execute(rusqlite::params![
                    inst.id,
                    pos as i32,
                    param.name,
                    value
                ])?;
            }
        }
    }
    Ok(())
}

pub(super) fn save_sends(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instrument_sends (instrument_id, bus_id, level, enabled)
             VALUES (?1, ?2, ?3, ?4)",
    )?;
    for inst in &instruments.instruments {
        for send in &inst.sends {
            stmt.execute(rusqlite::params![
                inst.id,
                send.bus_id,
                send.level as f64,
                send.enabled
            ])?;
        }
    }
    Ok(())
}

pub(super) fn save_modulations(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instrument_modulations (instrument_id, target_param, mod_type,
             lfo_rate, lfo_depth, env_attack, env_decay, env_sustain, env_release,
             source_instrument_id, source_param_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;

    for inst in &instruments.instruments {
        if let Some(ref f) = inst.filter {
            if let Some(ref ms) = f.cutoff.mod_source {
                insert_mod_source(&mut stmt, inst.id, "cutoff", ms)?;
            }
            if let Some(ref ms) = f.resonance.mod_source {
                insert_mod_source(&mut stmt, inst.id, "resonance", ms)?;
            }
        }
    }
    Ok(())
}

fn insert_mod_source(
    stmt: &mut rusqlite::Statement,
    instrument_id: InstrumentId,
    target: &str,
    ms: &ModSource,
) -> SqlResult<()> {
    match ms {
        ModSource::Lfo(lfo) => stmt.execute(rusqlite::params![
            instrument_id, target, "lfo",
            lfo.rate as f64, lfo.depth as f64,
            None::<f64>, None::<f64>, None::<f64>, None::<f64>,
            None::<i32>, None::<String>
        ]),
        ModSource::Envelope(env) => stmt.execute(rusqlite::params![
            instrument_id, target, "envelope",
            None::<f64>, None::<f64>,
            env.attack as f64, env.decay as f64, env.sustain as f64, env.release as f64,
            None::<i32>, None::<String>
        ]),
        ModSource::InstrumentParam(sid, name) => stmt.execute(rusqlite::params![
            instrument_id, target, "instrument_param",
            None::<f64>, None::<f64>,
            None::<f64>, None::<f64>, None::<f64>, None::<f64>,
            *sid, name
        ]),
    }?;
    Ok(())
}

pub(super) fn save_mixer(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
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

pub(super) fn save_sampler_configs(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut config_stmt = conn.prepare(
        "INSERT INTO sampler_configs (instrument_id, buffer_id, sample_name, loop_mode, pitch_tracking, next_slice_id, selected_slice)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut slice_stmt = conn.prepare(
        "INSERT INTO sampler_slices (instrument_id, slice_id, position, start_pos, end_pos, name, root_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for inst in &instruments.instruments {
        if let Some(ref config) = inst.sampler_config {
            config_stmt.execute(rusqlite::params![
                inst.id,
                config.buffer_id.map(|id| id as i32),
                config.sample_name,
                config.loop_mode,
                config.pitch_tracking,
                config.next_slice_id() as i32,
                config.selected_slice as i32,
            ])?;

            for (pos, slice) in config.slices.iter().enumerate() {
                slice_stmt.execute(rusqlite::params![
                    inst.id,
                    slice.id as i32,
                    pos as i32,
                    slice.start as f64,
                    slice.end as f64,
                    &slice.name,
                    slice.root_note as i32,
                ])?;
            }
        }
    }
    Ok(())
}

pub(super) fn save_automation(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    let mut lane_stmt = conn.prepare(
        "INSERT INTO automation_lanes (id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, min_value, max_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
            lane.min_value as f64,
            lane.max_value as f64,
        ])?;

        for point in &lane.points {
            let curve_str = match point.curve {
                super::super::automation::CurveType::Linear => "linear",
                super::super::automation::CurveType::Exponential => "exponential",
                super::super::automation::CurveType::Step => "step",
                super::super::automation::CurveType::SCurve => "scurve",
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

pub(super) fn save_piano_roll(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
    // Tracks
    {
        let mut stmt = conn.prepare(
            "INSERT INTO piano_roll_tracks (instrument_id, position, polyphonic)
                 VALUES (?1, ?2, ?3)",
        )?;
        for (pos, &sid) in session.piano_roll.track_order.iter().enumerate() {
            if let Some(track) = session.piano_roll.tracks.get(&sid) {
                stmt.execute(rusqlite::params![sid, pos as i32, track.polyphonic])?;
            }
        }
    }

    // Notes
    {
        let mut stmt = conn.prepare(
            "INSERT INTO piano_roll_notes (track_instrument_id, tick, duration, pitch, velocity)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for track in session.piano_roll.tracks.values() {
            for note in &track.notes {
                stmt.execute(rusqlite::params![
                    track.module_id,
                    note.tick,
                    note.duration,
                    note.pitch,
                    note.velocity
                ])?;
            }
        }
    }

    // Musical settings
    conn.execute(
        "INSERT INTO musical_settings (id, bpm, time_sig_num, time_sig_denom, ticks_per_beat, loop_start, loop_end, looping, key, scale, tuning_a4, snap)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            session.bpm as f64,
            session.time_signature.0,
            session.time_signature.1,
            session.piano_roll.ticks_per_beat,
            session.piano_roll.loop_start,
            session.piano_roll.loop_end,
            session.piano_roll.looping,
            session.key.name(),
            session.scale.name(),
            session.tuning_a4 as f64,
            session.snap,
        ],
    )?;
    Ok(())
}

pub(super) fn save_custom_synthdefs(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
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

pub(super) fn save_vst_plugins(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
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

pub(super) fn save_drum_sequencers(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut pad_stmt = conn.prepare(
        "INSERT INTO drum_pads (instrument_id, pad_index, buffer_id, path, name, level)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut pattern_stmt = conn.prepare(
        "INSERT INTO drum_patterns (instrument_id, pattern_index, length) VALUES (?1, ?2, ?3)",
    )?;
    let mut step_stmt = conn.prepare(
        "INSERT INTO drum_steps (instrument_id, pattern_index, pad_index, step_index, velocity)
             VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for inst in &instruments.instruments {
        if let Some(seq) = &inst.drum_sequencer {
            let instrument_id = inst.id as i32;

            for (i, pad) in seq.pads.iter().enumerate() {
                pad_stmt.execute(rusqlite::params![
                    instrument_id,
                    i,
                    pad.buffer_id.map(|id| id as i32),
                    pad.path,
                    pad.name,
                    pad.level as f64,
                ])?;
            }

            for (pi, pattern) in seq.patterns.iter().enumerate() {
                pattern_stmt.execute(rusqlite::params![instrument_id, pi, pattern.length])?;

                for (pad_idx, pad_steps) in pattern.steps.iter().enumerate() {
                    for (step_idx, step) in pad_steps.iter().enumerate() {
                        if step.active {
                            step_stmt.execute(rusqlite::params![
                                instrument_id, pi, pad_idx, step_idx, step.velocity as i32
                            ])?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn save_chopper_states(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut header_stmt = conn.prepare(
        "INSERT INTO chopper_states (instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut slice_stmt = conn.prepare(
        "INSERT INTO chopper_slices (instrument_id, slice_id, position, start_pos, end_pos, name, root_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for inst in &instruments.instruments {
        if let Some(seq) = &inst.drum_sequencer {
            if let Some(chopper) = &seq.chopper {
                let instrument_id = inst.id as i32;

                header_stmt.execute(rusqlite::params![
                    instrument_id,
                    chopper.buffer_id.map(|id| id as i32),
                    chopper.path,
                    chopper.name,
                    chopper.selected_slice as i32,
                    chopper.next_slice_id as i32,
                    chopper.duration_secs as f64,
                ])?;

                for (pos, slice) in chopper.slices.iter().enumerate() {
                    slice_stmt.execute(rusqlite::params![
                        instrument_id,
                        slice.id as i32,
                        pos as i32,
                        slice.start as f64,
                        slice.end as f64,
                        &slice.name,
                        slice.root_note as i32,
                    ])?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn save_midi_recording(conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
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
