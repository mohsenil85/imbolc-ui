use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::*;
use crate::state::instrument_state::InstrumentState;
use crate::state::param::ParamValue;

pub(crate) fn save_instruments(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instruments (id, name, position, source_type, filter_type, filter_cutoff, filter_resonance,
             lfo_enabled, lfo_rate, lfo_depth, lfo_shape, lfo_target,
             amp_attack, amp_decay, amp_sustain, amp_release, polyphonic,
             level, pan, mute, solo, active, output_target, vst_state_path,
             arp_enabled, arp_direction, arp_rate, arp_octaves, arp_gate, chord_shape, convolution_ir_path, eq_enabled, layer_group)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)",
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
            inst.vst_state_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            inst.arpeggiator.enabled,
            inst.arpeggiator.direction.name(),
            inst.arpeggiator.rate.name(),
            inst.arpeggiator.octaves as i32,
            inst.arpeggiator.gate as f64,
            inst.chord_shape.as_ref().map(|s| s.name()),
            inst.convolution_ir_path.as_deref(),
            inst.eq.as_ref().map(|_| 1i32),
            inst.layer_group.map(|g| g as i32),
        ])?;
    }
    Ok(())
}

pub(crate) fn save_eq_bands(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO instrument_eq_bands (instrument_id, band_index, band_type, freq, gain, q, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for inst in &instruments.instruments {
        if let Some(ref eq) = inst.eq {
            for (i, band) in eq.bands.iter().enumerate() {
                let band_type_str = match band.band_type {
                    EqBandType::LowShelf => "lowshelf",
                    EqBandType::Peaking => "peaking",
                    EqBandType::HighShelf => "highshelf",
                };
                stmt.execute(rusqlite::params![
                    inst.id,
                    i as i32,
                    band_type_str,
                    band.freq as f64,
                    band.gain as f64,
                    band.q as f64,
                    band.enabled,
                ])?;
            }
        }
    }
    Ok(())
}

pub(crate) fn save_source_params(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
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

pub(crate) fn save_effects(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
    let mut effect_stmt = conn.prepare(
        "INSERT INTO instrument_effects (instrument_id, position, effect_type, enabled, vst_state_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
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
                effect.enabled,
                effect.vst_state_path.as_ref().map(|p| p.to_string_lossy().to_string()),
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

pub(crate) fn save_sends(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
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

pub(crate) fn save_modulations(conn: &SqlConnection, instruments: &InstrumentState) -> SqlResult<()> {
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
