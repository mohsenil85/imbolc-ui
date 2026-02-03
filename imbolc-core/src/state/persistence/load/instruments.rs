use std::path::PathBuf;
use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::*;
use crate::state::param::{Param, ParamValue};
use crate::state::session::MAX_BUSES;
use crate::state::persistence::conversion::{
    parse_effect_type, parse_filter_type, parse_source_type,
};

pub(crate) fn load_instruments(conn: &SqlConnection) -> SqlResult<Vec<Instrument>> {
    let mut instruments = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT id, name, source_type, filter_type, filter_cutoff, filter_resonance,
         COALESCE(lfo_enabled, 0) as lfo_enabled,
         COALESCE(lfo_rate, 2.0) as lfo_rate,
         COALESCE(lfo_depth, 0.5) as lfo_depth,
         COALESCE(lfo_shape, 'sine') as lfo_shape,
         COALESCE(lfo_target, 'filter') as lfo_target,
         amp_attack, amp_decay, amp_sustain, amp_release, polyphonic,
         level, pan, mute, solo, COALESCE(active, 1) as active, output_target
         FROM instruments ORDER BY position",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: InstrumentId = row.get(0)?;
        let name: String = row.get(1)?;
        let source_str: String = row.get(2)?;
        let filter_type_str: Option<String> = row.get(3)?;
        let filter_cutoff: Option<f64> = row.get(4)?;
        let filter_res: Option<f64> = row.get(5)?;
        let lfo_enabled: bool = row.get(6)?;
        let lfo_rate: f64 = row.get(7)?;
        let lfo_depth: f64 = row.get(8)?;
        let lfo_shape_str: String = row.get(9)?;
        let lfo_target_str: String = row.get(10)?;
        let attack: f64 = row.get(11)?;
        let decay: f64 = row.get(12)?;
        let sustain: f64 = row.get(13)?;
        let release: f64 = row.get(14)?;
        let polyphonic: bool = row.get(15)?;
        let level: f64 = row.get(16)?;
        let pan: f64 = row.get(17)?;
        let mute: bool = row.get(18)?;
        let solo: bool = row.get(19)?;
        let active: bool = row.get(20)?;
        let output_str: String = row.get(21)?;
        Ok((
            id, name, source_str, filter_type_str, filter_cutoff, filter_res,
            lfo_enabled, lfo_rate, lfo_depth, lfo_shape_str, lfo_target_str,
            attack, decay, sustain, release, polyphonic,
            level, pan, mute, solo, active, output_str,
        ))
    })?;

    for result in rows {
        let (
            id, name, source_str, filter_type_str, filter_cutoff, filter_res,
            lfo_enabled, lfo_rate, lfo_depth, lfo_shape_str, lfo_target_str,
            attack, decay, sustain, release, polyphonic,
            level, pan, mute, solo, active, output_str,
        ) = result?;

        let source = parse_source_type(&source_str);
        let filter = filter_type_str.map(|ft| {
            let filter_type = parse_filter_type(&ft);
            let mut config = FilterConfig::new(filter_type);
            if let Some(c) = filter_cutoff {
                config.cutoff.value = c as f32;
            }
            if let Some(r) = filter_res {
                config.resonance.value = r as f32;
            }
            config
        });
        let lfo_shape = match lfo_shape_str.as_str() {
            "square" => LfoShape::Square,
            "saw" => LfoShape::Saw,
            "triangle" => LfoShape::Triangle,
            _ => LfoShape::Sine,
        };
        let lfo_target = match lfo_target_str.as_str() {
            "filter_cutoff" | "filter" => LfoTarget::FilterCutoff,
            "filter_res" => LfoTarget::FilterResonance,
            "amp" => LfoTarget::Amplitude,
            "pitch" => LfoTarget::Pitch,
            "pan" => LfoTarget::Pan,
            "pulse_width" => LfoTarget::PulseWidth,
            "sample_rate" => LfoTarget::SampleRate,
            "delay_time" => LfoTarget::DelayTime,
            "delay_feedback" => LfoTarget::DelayFeedback,
            "reverb_mix" => LfoTarget::ReverbMix,
            "gate_rate" => LfoTarget::GateRate,
            "send_level" => LfoTarget::SendLevel,
            "detune" => LfoTarget::Detune,
            "attack" => LfoTarget::Attack,
            "release" => LfoTarget::Release,
            "fm_index" => LfoTarget::FmIndex,
            "wavetable_position" => LfoTarget::WavetablePosition,
            "formant_freq" => LfoTarget::FormantFreq,
            "sync_ratio" => LfoTarget::SyncRatio,
            "pressure" => LfoTarget::Pressure,
            "embouchure" => LfoTarget::Embouchure,
            "grain_size" => LfoTarget::GrainSize,
            "grain_density" => LfoTarget::GrainDensity,
            "fb_feedback" => LfoTarget::FbFeedback,
            "ring_mod_depth" => LfoTarget::RingModDepth,
            "chaos_param" => LfoTarget::ChaosParam,
            "additive_rolloff" => LfoTarget::AdditiveRolloff,
            "membrane_tension" => LfoTarget::MembraneTension,
            "decay" => LfoTarget::Decay,
            "sustain" => LfoTarget::Sustain,
            _ => LfoTarget::FilterCutoff,
        };
        let output_target = if output_str == "master" {
            OutputTarget::Master
        } else if let Some(n) = output_str.strip_prefix("bus:") {
            n.parse::<u8>()
                .map(OutputTarget::Bus)
                .unwrap_or(OutputTarget::Master)
        } else {
            OutputTarget::Master
        };

        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        let sampler_config = if source.is_sample() {
            Some(crate::state::sampler::SamplerConfig::default())
        } else {
            None
        };
        let drum_sequencer = if source.is_kit() {
            Some(crate::state::drum_sequencer::DrumSequencerState::new())
        } else {
            None
        };

        instruments.push(Instrument {
            id, name, source,
            source_params: source.default_params(),
            filter,
            eq: None,
            effects: Vec::new(),
            lfo: LfoConfig {
                enabled: lfo_enabled,
                rate: lfo_rate as f32,
                depth: lfo_depth as f32,
                shape: lfo_shape,
                target: lfo_target,
            },
            amp_envelope: EnvConfig {
                attack: attack as f32,
                decay: decay as f32,
                sustain: sustain as f32,
                release: release as f32,
            },
            polyphonic,
            level: level as f32,
            pan: pan as f32,
            mute, solo, active, output_target, sends,
            sampler_config, drum_sequencer,
            vst_param_values: Vec::new(),
            vst_state_path: None,
            arpeggiator: crate::state::arpeggiator::ArpeggiatorConfig::default(),
            chord_shape: None,
            convolution_ir_path: None,
            layer_group: None,
        });
    }
    Ok(instruments)
}

pub(crate) fn load_eq_bands(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    // First, check if instruments have eq_enabled flag and create default EQ configs
    let has_eq_col = conn
        .prepare("SELECT eq_enabled FROM instruments LIMIT 0")
        .is_ok();
    if has_eq_col {
        let mut eq_stmt = conn.prepare(
            "SELECT id, eq_enabled FROM instruments WHERE eq_enabled IS NOT NULL",
        )?;
        let eq_instruments: Vec<InstrumentId> = eq_stmt
            .query_map([], |row| {
                let id: InstrumentId = row.get(0)?;
                Ok(id)
            })?
            .filter_map(|r| r.ok())
            .collect();
        for inst in instruments.iter_mut() {
            if eq_instruments.contains(&inst.id) {
                inst.eq = Some(EqConfig::default());
            }
        }
    }

    // Load band data from instrument_eq_bands table
    let has_table = conn
        .prepare("SELECT 1 FROM instrument_eq_bands LIMIT 0")
        .is_ok();
    if !has_table {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT band_index, band_type, freq, gain, q, enabled
         FROM instrument_eq_bands WHERE instrument_id = ?1 ORDER BY band_index",
    )?;
    for inst in instruments {
        if let Some(ref mut eq) = inst.eq {
            let rows: Vec<(i32, String, f64, f64, f64, bool)> = stmt
                .query_map([&inst.id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            for (band_idx, band_type_str, freq, gain, q, enabled) in rows {
                let idx = band_idx as usize;
                if idx < eq.bands.len() {
                    let band_type = match band_type_str.as_str() {
                        "lowshelf" => EqBandType::LowShelf,
                        "highshelf" => EqBandType::HighShelf,
                        _ => EqBandType::Peaking,
                    };
                    eq.bands[idx] = EqBand {
                        band_type,
                        freq: freq as f32,
                        gain: gain as f32,
                        q: q as f32,
                        enabled,
                    };
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_source_params(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "SELECT param_name, param_value, param_min, param_max, param_type
         FROM instrument_source_params WHERE instrument_id = ?1",
    )?;
    for inst in instruments {
        let params: Vec<Param> = stmt
            .query_map([&inst.id], |row| {
                let name: String = row.get(0)?;
                let value: f64 = row.get(1)?;
                let min: f64 = row.get(2)?;
                let max: f64 = row.get(3)?;
                let param_type: String = row.get(4)?;
                Ok((name, value, min, max, param_type))
            })?
            .filter_map(|r| r.ok())
            .map(|(name, value, min, max, param_type)| {
                let pv = match param_type.as_str() {
                    "int" => ParamValue::Int(value as i32),
                    "bool" => ParamValue::Bool(value != 0.0),
                    _ => ParamValue::Float(value as f32),
                };
                Param { name, value: pv, min: min as f32, max: max as f32 }
            })
            .collect();
        if !params.is_empty() {
            inst.source_params = params;
        }
    }
    Ok(())
}

pub(crate) fn load_effects(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    // Check if vst_state_path column exists (backwards compat)
    let has_vst_state_path = conn
        .prepare("SELECT vst_state_path FROM instrument_effects LIMIT 0")
        .is_ok();
    let effects_query = if has_vst_state_path {
        "SELECT position, effect_type, enabled, vst_state_path FROM instrument_effects WHERE instrument_id = ?1 ORDER BY position"
    } else {
        "SELECT position, effect_type, enabled, NULL FROM instrument_effects WHERE instrument_id = ?1 ORDER BY position"
    };
    let mut effect_stmt = conn.prepare(effects_query)?;
    let mut param_stmt = conn.prepare(
        "SELECT param_name, param_value FROM instrument_effect_params WHERE instrument_id = ?1 AND effect_position = ?2",
    )?;
    for inst in instruments {
        let effects: Vec<(i32, String, bool, Option<String>)> = effect_stmt
            .query_map([&inst.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (pos, type_str, enabled, vst_state_path_str) in effects {
            let effect_type = parse_effect_type(&type_str);
            let mut slot = EffectSlot::new(effect_type);
            slot.enabled = enabled;
            slot.vst_state_path = vst_state_path_str.map(PathBuf::from);

            let params: Vec<(String, f64)> = param_stmt
                .query_map(rusqlite::params![inst.id, pos], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            for (name, value) in params {
                if let Some(p) = slot.params.iter_mut().find(|p| p.name == name) {
                    p.value = match &p.value {
                        ParamValue::Int(_) => ParamValue::Int(value as i32),
                        ParamValue::Bool(_) => ParamValue::Bool(value != 0.0),
                        _ => ParamValue::Float(value as f32),
                    };
                }
            }

            inst.effects.push(slot);
        }
    }
    Ok(())
}

pub(crate) fn load_sends(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, bus_id, level, enabled FROM instrument_sends",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let instrument_id: InstrumentId = row.get(0)?;
            let bus_id: u8 = row.get(1)?;
            let level: f64 = row.get(2)?;
            let enabled: bool = row.get(3)?;
            Ok((instrument_id, bus_id, level, enabled))
        }) {
            for result in rows {
                if let Ok((instrument_id, bus_id, level, enabled)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(send) = inst.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                            send.level = level as f32;
                            send.enabled = enabled;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_modulations(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, target_param, mod_type, lfo_rate, lfo_depth,
         env_attack, env_decay, env_sustain, env_release,
         source_instrument_id, source_param_name
         FROM instrument_modulations",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<f64>>(4)?,
                row.get::<_, Option<f64>>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<f64>>(8)?,
                row.get::<_, Option<InstrumentId>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, target, mod_type, lfo_rate, lfo_depth, env_a, env_d, env_s, env_r, src_id, src_name)) = result {
                    let mod_source = match mod_type.as_str() {
                        "lfo" => Some(ModSource::Lfo(LfoConfig {
                            enabled: true,
                            rate: lfo_rate.unwrap_or(1.0) as f32,
                            depth: lfo_depth.unwrap_or(0.5) as f32,
                            shape: LfoShape::Sine,
                            target: LfoTarget::FilterCutoff,
                        })),
                        "envelope" => Some(ModSource::Envelope(EnvConfig {
                            attack: env_a.unwrap_or(0.01) as f32,
                            decay: env_d.unwrap_or(0.1) as f32,
                            sustain: env_s.unwrap_or(0.7) as f32,
                            release: env_r.unwrap_or(0.3) as f32,
                        })),
                        "instrument_param" => {
                            src_id.zip(src_name).map(|(id, name)| ModSource::InstrumentParam(id, name))
                        }
                        _ => None,
                    };

                    if let Some(ms) = mod_source {
                        if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                            if let Some(ref mut f) = inst.filter {
                                match target.as_str() {
                                    "cutoff" => f.cutoff.mod_source = Some(ms),
                                    "resonance" => f.resonance.mod_source = Some(ms),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_filter_params(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    let has_table = conn
        .prepare("SELECT 1 FROM instrument_filter_params LIMIT 0")
        .is_ok();
    if !has_table {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT param_name, param_value, param_min, param_max, param_type
         FROM instrument_filter_params WHERE instrument_id = ?1",
    )?;
    for inst in instruments {
        if let Some(ref mut f) = inst.filter {
            let params: Vec<(String, f64, f64, f64, String)> = stmt
                .query_map([&inst.id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            for (name, value, min, max, param_type) in params {
                if let Some(p) = f.extra_params.iter_mut().find(|p| p.name == name) {
                    p.value = match param_type.as_str() {
                        "int" => ParamValue::Int(value as i32),
                        "bool" => ParamValue::Bool(value != 0.0),
                        _ => ParamValue::Float(value as f32),
                    };
                    p.min = min as f32;
                    p.max = max as f32;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_layer_groups(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    let has_col = conn
        .prepare("SELECT layer_group FROM instruments LIMIT 0")
        .is_ok();
    if !has_col {
        return Ok(());
    }
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, layer_group FROM instruments WHERE layer_group IS NOT NULL",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, i32>(1)?))
        }) {
            for result in rows {
                if let Ok((id, group)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == id) {
                        inst.layer_group = Some(group as u32);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn load_arpeggiator_settings(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    use crate::state::arpeggiator::{ArpDirection, ArpRate, ArpeggiatorConfig, ChordShape};

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, arp_enabled, arp_direction, arp_rate, arp_octaves, arp_gate, chord_shape, convolution_ir_path FROM instruments ORDER BY position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, bool>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        }) {
            for result in rows {
                if let Ok((id, enabled, dir_str, rate_str, octaves, gate, chord_str, ir_path)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == id) {
                        let direction = match dir_str.as_str() {
                            "Down" => ArpDirection::Down,
                            "Up/Down" => ArpDirection::UpDown,
                            "Random" => ArpDirection::Random,
                            _ => ArpDirection::Up,
                        };
                        let rate = match rate_str.as_str() {
                            "1/4" => ArpRate::Quarter,
                            "1/16" => ArpRate::Sixteenth,
                            "1/32" => ArpRate::ThirtySecond,
                            _ => ArpRate::Eighth,
                        };
                        inst.arpeggiator = ArpeggiatorConfig {
                            enabled,
                            direction,
                            rate,
                            octaves: octaves.clamp(1, 4) as u8,
                            gate: gate as f32,
                        };
                        inst.chord_shape = chord_str.and_then(|s| match s.as_str() {
                            "Major" => Some(ChordShape::Major),
                            "Minor" => Some(ChordShape::Minor),
                            "7th" => Some(ChordShape::Seventh),
                            "m7" => Some(ChordShape::MinorSeventh),
                            "sus2" => Some(ChordShape::Sus2),
                            "sus4" => Some(ChordShape::Sus4),
                            "Power" => Some(ChordShape::PowerChord),
                            "Octave" => Some(ChordShape::Octave),
                            _ => None,
                        });
                        inst.convolution_ir_path = ir_path;
                    }
                }
            }
        }
    }
    Ok(())
}
