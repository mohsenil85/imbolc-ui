use std::path::PathBuf;

use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use super::super::custom_synthdef::{CustomSynthDef, CustomSynthDefRegistry, ParamSpec};
use super::super::instrument::*;
use super::super::param::{Param, ParamValue};
use super::super::piano_roll::PianoRollState;
use super::super::session::MAX_BUSES;
use super::super::vst_plugin::{VstParamSpec, VstPlugin, VstPluginKind, VstPluginRegistry};
use super::conversion::{
    deserialize_automation_target, parse_effect_type, parse_filter_type, parse_key, parse_scale,
    parse_source_type,
};

/// Musical settings loaded from the database, used to populate SessionState fields.
pub(super) struct MusicalSettingsLoaded {
    pub bpm: u16,
    pub time_signature: (u8, u8),
    pub key: super::super::music::Key,
    pub scale: super::super::music::Scale,
    pub tuning_a4: f32,
    pub snap: bool,
    pub humanize_velocity: f32,
    pub humanize_timing: f32,
}

impl Default for MusicalSettingsLoaded {
    fn default() -> Self {
        Self {
            bpm: 120,
            time_signature: (4, 4),
            key: super::super::music::Key::C,
            scale: super::super::music::Scale::Major,
            tuning_a4: 440.0,
            snap: false,
            humanize_velocity: 0.0,
            humanize_timing: 0.0,
        }
    }
}

pub(super) fn load_instruments(conn: &SqlConnection) -> SqlResult<Vec<Instrument>> {
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
            Some(super::super::sampler::SamplerConfig::default())
        } else {
            None
        };
        let drum_sequencer = if source.is_kit() {
            Some(super::super::drum_sequencer::DrumSequencerState::new())
        } else {
            None
        };

        instruments.push(Instrument {
            id, name, source,
            source_params: source.default_params(),
            filter,
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
        });
    }
    Ok(instruments)
}

pub(super) fn load_source_params(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_effects(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    let mut effect_stmt = conn.prepare(
        "SELECT position, effect_type, enabled FROM instrument_effects WHERE instrument_id = ?1 ORDER BY position",
    )?;
    let mut param_stmt = conn.prepare(
        "SELECT param_name, param_value FROM instrument_effect_params WHERE instrument_id = ?1 AND effect_position = ?2",
    )?;
    for inst in instruments {
        let effects: Vec<(i32, String, bool)> = effect_stmt
            .query_map([&inst.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (pos, type_str, enabled) in effects {
            let effect_type = parse_effect_type(&type_str);
            let mut slot = EffectSlot::new(effect_type);
            slot.enabled = enabled;

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

pub(super) fn load_sends(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_modulations(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_buses(conn: &SqlConnection) -> SqlResult<Vec<MixerBus>> {
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

pub(super) fn load_master(conn: &SqlConnection) -> (f32, bool) {
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

pub(super) fn load_piano_roll(conn: &SqlConnection) -> SqlResult<(PianoRollState, MusicalSettingsLoaded)> {
    let mut piano_roll = PianoRollState::new();
    let mut musical = MusicalSettingsLoaded::default();

    if let Ok(row) = conn.query_row(
        "SELECT bpm, time_sig_num, time_sig_denom, ticks_per_beat, loop_start, loop_end, looping, key, scale, tuning_a4, snap
         FROM musical_settings WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, f64>(0)?, row.get::<_, u8>(1)?, row.get::<_, u8>(2)?,
                row.get::<_, u32>(3)?, row.get::<_, u32>(4)?, row.get::<_, u32>(5)?,
                row.get::<_, bool>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                row.get::<_, f64>(9)?, row.get::<_, bool>(10)?,
            ))
        },
    ) {
        musical.bpm = row.0 as u16;
        musical.time_signature = (row.1, row.2);
        musical.key = parse_key(&row.7);
        musical.scale = parse_scale(&row.8);
        musical.tuning_a4 = row.9 as f32;
        musical.snap = row.10;
        piano_roll.bpm = row.0 as f32;
        piano_roll.time_signature = (row.1, row.2);
        piano_roll.ticks_per_beat = row.3;
        piano_roll.loop_start = row.4;
        piano_roll.loop_end = row.5;
        piano_roll.looping = row.6;
    }

    // Load swing_amount (optional, may not exist in older schemas)
    if let Ok(swing) = conn.query_row(
        "SELECT swing_amount FROM musical_settings WHERE id = 1",
        [],
        |row| row.get::<_, f64>(0),
    ) {
        piano_roll.swing_amount = swing as f32;
    }

    // Load humanization settings (optional, may not exist in older schemas)
    if let Ok((hv, ht)) = conn.query_row(
        "SELECT humanize_velocity, humanize_timing FROM musical_settings WHERE id = 1",
        [],
        |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
    ) {
        musical.humanize_velocity = hv as f32;
        musical.humanize_timing = ht as f32;
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, polyphonic FROM piano_roll_tracks ORDER BY position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, bool>(1)?))
        }) {
            for result in rows {
                if let Ok((instrument_id, polyphonic)) = result {
                    piano_roll.track_order.push(instrument_id);
                    piano_roll.tracks.insert(
                        instrument_id,
                        super::super::piano_roll::Track {
                            module_id: instrument_id,
                            notes: Vec::new(),
                            polyphonic,
                        },
                    );
                }
            }
        }
    }

    let has_probability = conn
        .prepare("SELECT probability FROM piano_roll_notes LIMIT 0")
        .is_ok();
    let notes_query = if has_probability {
        "SELECT track_instrument_id, tick, duration, pitch, velocity, probability FROM piano_roll_notes"
    } else {
        "SELECT track_instrument_id, tick, duration, pitch, velocity FROM piano_roll_notes"
    };
    if let Ok(mut stmt) = conn.prepare(notes_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let probability = if has_probability { row.get::<_, f32>(5).unwrap_or(1.0) } else { 1.0 };
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, u8>(3)?,
                row.get::<_, u8>(4)?,
                probability,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, tick, duration, pitch, velocity, probability)) = result {
                    if let Some(track) = piano_roll.tracks.get_mut(&instrument_id) {
                        track.notes.push(super::super::piano_roll::Note { tick, duration, pitch, velocity, probability });
                    }
                }
            }
        }
    }

    Ok((piano_roll, musical))
}

pub(super) fn load_sampler_configs(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    let has_sample_name = conn
        .prepare("SELECT sample_name FROM sampler_configs LIMIT 0")
        .is_ok();
    let query = if has_sample_name {
        "SELECT instrument_id, buffer_id, loop_mode, pitch_tracking, next_slice_id, COALESCE(selected_slice, 0), sample_name FROM sampler_configs"
    } else {
        "SELECT instrument_id, buffer_id, loop_mode, pitch_tracking, next_slice_id, COALESCE(selected_slice, 0), NULL FROM sampler_configs"
    };
    if let Ok(mut config_stmt) = conn.prepare(query) {
        if let Ok(rows) = config_stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, Option<i32>>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, i32>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, buffer_id, loop_mode, pitch_tracking, next_slice_id, selected_slice, sample_name)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(ref mut config) = inst.sampler_config {
                            config.buffer_id = buffer_id.map(|id| id as super::super::sampler::BufferId);
                            config.sample_name = sample_name;
                            config.loop_mode = loop_mode;
                            config.pitch_tracking = pitch_tracking;
                            config.set_next_slice_id(next_slice_id as super::super::sampler::SliceId);
                            config.selected_slice = selected_slice as usize;
                            config.slices.clear();
                        }
                    }
                }
            }
        }
    }

    if let Ok(mut slice_stmt) = conn.prepare(
        "SELECT instrument_id, slice_id, start_pos, end_pos, name, root_note FROM sampler_slices ORDER BY instrument_id, position",
    ) {
        if let Ok(rows) = slice_stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, slice_id, start, end, name, root_note)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(ref mut config) = inst.sampler_config {
                            config.slices.push(super::super::sampler::Slice {
                                id: slice_id as super::super::sampler::SliceId,
                                start: start as f32,
                                end: end as f32,
                                name,
                                root_note: root_note as u8,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub(super) fn load_automation(conn: &SqlConnection) -> SqlResult<super::super::automation::AutomationState> {
    use super::super::automation::{AutomationLane, AutomationPoint, AutomationState, CurveType};

    let mut state = AutomationState::new();

    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, target_type, target_instrument_id, target_effect_idx, target_param_idx, enabled, min_value, max_value
         FROM automation_lanes",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?, row.get::<_, String>(1)?,
                row.get::<_, InstrumentId>(2)?, row.get::<_, Option<i32>>(3)?,
                row.get::<_, Option<i32>>(4)?, row.get::<_, bool>(5)?,
                row.get::<_, f64>(6)?, row.get::<_, f64>(7)?,
            ))
        }) {
            for result in rows {
                if let Ok((id, target_type, instrument_id, effect_idx, param_idx, enabled, min_value, max_value)) = result {
                    if let Some(target) = deserialize_automation_target(&target_type, instrument_id, effect_idx, param_idx) {
                        let mut lane = AutomationLane::new(id as u32, target);
                        lane.enabled = enabled;
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

pub(super) fn load_drum_sequencers(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    use super::super::drum_sequencer::DrumPattern;

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pad_index, buffer_id, path, name, level FROM drum_pads",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?,
                row.get::<_, Option<u32>>(2)?, row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?, row.get::<_, f64>(5)?,
            ))
        }) {
            for row in rows {
                if let Ok((instrument_id, idx, buffer_id, path, name, level)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pad) = seq.pads.get_mut(idx) {
                                pad.buffer_id = buffer_id;
                                pad.path = path;
                                pad.name = name;
                                pad.level = level as f32;
                            }
                        }
                    }
                }
            }
        }
    }

    for inst in instruments.iter_mut() {
        if let Some(seq) = &mut inst.drum_sequencer {
            let max_id = seq.pads.iter().filter_map(|p| p.buffer_id).max().unwrap_or(9999);
            seq.next_buffer_id = max_id + 1;
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pattern_index, length FROM drum_patterns ORDER BY instrument_id, pattern_index",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?, row.get::<_, usize>(2)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, idx, length)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pattern) = seq.patterns.get_mut(idx) {
                                *pattern = DrumPattern::new(length);
                            }
                        }
                    }
                }
            }
        }
    }

    // Load swing_amount (optional, may not exist in older schemas)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, swing_amount FROM drum_patterns WHERE pattern_index = 0",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, f64>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, swing)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.swing_amount = swing as f32;
                        }
                    }
                }
            }
        }
    }

    let has_step_probability = conn
        .prepare("SELECT probability FROM drum_steps LIMIT 0")
        .is_ok();
    let steps_query = if has_step_probability {
        "SELECT instrument_id, pattern_index, pad_index, step_index, velocity, probability FROM drum_steps"
    } else {
        "SELECT instrument_id, pattern_index, pad_index, step_index, velocity FROM drum_steps"
    };
    if let Ok(mut stmt) = conn.prepare(steps_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let probability = if has_step_probability { row.get::<_, f32>(5).unwrap_or(1.0) } else { 1.0 };
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?,
                row.get::<_, usize>(2)?, row.get::<_, usize>(3)?,
                row.get::<_, u8>(4)?, probability,
            ))
        }) {
            for row in rows {
                if let Ok((instrument_id, pi, pad_idx, step_idx, velocity, probability)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(pattern) = seq.patterns.get_mut(pi) {
                                if let Some(step) = pattern.steps.get_mut(pad_idx).and_then(|s| s.get_mut(step_idx)) {
                                    step.active = true;
                                    step.velocity = velocity;
                                    step.probability = probability;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Load chain_enabled (optional, may not exist in older schemas)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, chain_enabled FROM drum_patterns WHERE pattern_index = 0",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, bool>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, chain_enabled)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chain_enabled = chain_enabled;
                        }
                    }
                }
            }
        }
    }

    // Load pattern chain
    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, pattern_index FROM drum_sequencer_chain ORDER BY instrument_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, InstrumentId>(0)?, row.get::<_, usize>(1)?))
        }) {
            for row in rows {
                if let Ok((instrument_id, pattern_index)) = row {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chain.push(pattern_index);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub(super) fn load_chopper_states(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
    use super::super::drum_sequencer::ChopperState;
    use super::super::sampler::Slice;

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs
         FROM chopper_states",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, Option<i32>>(1)?,
                row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?,
                row.get::<_, usize>(4)?, row.get::<_, u32>(5)?,
                row.get::<_, f64>(6)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, buffer_id, path, name, selected_slice, next_slice_id, duration_secs)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            seq.chopper = Some(ChopperState {
                                buffer_id: buffer_id.map(|id| id as u32),
                                path, name,
                                slices: Vec::new(),
                                selected_slice, next_slice_id,
                                waveform_peaks: Vec::new(),
                                duration_secs: duration_secs as f32,
                            });
                        }
                    }
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare(
        "SELECT instrument_id, slice_id, start_pos, end_pos, name, root_note
         FROM chopper_slices ORDER BY instrument_id, position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, InstrumentId>(0)?, row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?, row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?, row.get::<_, i32>(5)?,
            ))
        }) {
            for result in rows {
                if let Ok((instrument_id, slice_id, start, end, name, root_note)) = result {
                    if let Some(inst) = instruments.iter_mut().find(|s| s.id == instrument_id) {
                        if let Some(seq) = &mut inst.drum_sequencer {
                            if let Some(chopper) = &mut seq.chopper {
                                chopper.slices.push(Slice {
                                    id: slice_id as u32,
                                    start: start as f32,
                                    end: end as f32,
                                    name,
                                    root_note: root_note as u8,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub(super) fn load_midi_recording(conn: &SqlConnection) -> SqlResult<super::super::midi_recording::MidiRecordingState> {
    use super::super::midi_recording::{MidiCcMapping, MidiRecordingState, PitchBendConfig};

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

    state.record_mode = super::super::midi_recording::RecordMode::Off;
    Ok(state)
}

pub(super) fn load_custom_synthdefs(conn: &SqlConnection) -> SqlResult<CustomSynthDefRegistry> {
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

pub(super) fn load_vst_state_paths(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_vst_param_values(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_arpeggiator_settings(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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

pub(super) fn load_vst_plugins(conn: &SqlConnection) -> SqlResult<VstPluginRegistry> {
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
