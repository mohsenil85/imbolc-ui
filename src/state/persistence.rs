use std::path::Path;

use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use super::music::{Key, Scale};
use super::param::{Param, ParamValue};
use super::piano_roll::PianoRollState;
use super::strip::*;
use super::strip_state::{MixerSelection, StripState};
use crate::ui::frame::SessionState;

impl StripState {
    /// Save to SQLite
    pub fn save(&self, path: &Path, session: &SessionState) -> SqlResult<()> {
        let conn = SqlConnection::open(path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                next_strip_id INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS strips (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                position INTEGER NOT NULL,
                source_type TEXT NOT NULL,
                filter_type TEXT,
                filter_cutoff REAL,
                filter_resonance REAL,
                lfo_enabled INTEGER NOT NULL DEFAULT 0,
                lfo_rate REAL NOT NULL DEFAULT 2.0,
                lfo_depth REAL NOT NULL DEFAULT 0.5,
                lfo_shape TEXT NOT NULL DEFAULT 'sine',
                lfo_target TEXT NOT NULL DEFAULT 'filter',
                amp_attack REAL NOT NULL,
                amp_decay REAL NOT NULL,
                amp_sustain REAL NOT NULL,
                amp_release REAL NOT NULL,
                polyphonic INTEGER NOT NULL,
                has_track INTEGER NOT NULL,
                level REAL NOT NULL,
                pan REAL NOT NULL,
                mute INTEGER NOT NULL,
                solo INTEGER NOT NULL,
                output_target TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS strip_source_params (
                strip_id INTEGER NOT NULL,
                param_name TEXT NOT NULL,
                param_value REAL NOT NULL,
                param_min REAL NOT NULL,
                param_max REAL NOT NULL,
                param_type TEXT NOT NULL,
                PRIMARY KEY (strip_id, param_name)
            );

            CREATE TABLE IF NOT EXISTS strip_effects (
                strip_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                effect_type TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                PRIMARY KEY (strip_id, position)
            );

            CREATE TABLE IF NOT EXISTS strip_effect_params (
                strip_id INTEGER NOT NULL,
                effect_position INTEGER NOT NULL,
                param_name TEXT NOT NULL,
                param_value REAL NOT NULL,
                PRIMARY KEY (strip_id, effect_position, param_name)
            );

            CREATE TABLE IF NOT EXISTS strip_sends (
                strip_id INTEGER NOT NULL,
                bus_id INTEGER NOT NULL,
                level REAL NOT NULL,
                enabled INTEGER NOT NULL,
                PRIMARY KEY (strip_id, bus_id)
            );

            CREATE TABLE IF NOT EXISTS strip_modulations (
                strip_id INTEGER NOT NULL,
                target_param TEXT NOT NULL,
                mod_type TEXT NOT NULL,
                lfo_rate REAL,
                lfo_depth REAL,
                env_attack REAL,
                env_decay REAL,
                env_sustain REAL,
                env_release REAL,
                source_strip_id INTEGER,
                source_param_name TEXT,
                PRIMARY KEY (strip_id, target_param)
            );

            CREATE TABLE IF NOT EXISTS mixer_buses (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                level REAL NOT NULL,
                pan REAL NOT NULL,
                mute INTEGER NOT NULL,
                solo INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS mixer_master (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                level REAL NOT NULL,
                mute INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS piano_roll_tracks (
                strip_id INTEGER PRIMARY KEY,
                position INTEGER NOT NULL,
                polyphonic INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS piano_roll_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_strip_id INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                duration INTEGER NOT NULL,
                pitch INTEGER NOT NULL,
                velocity INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS musical_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                bpm REAL NOT NULL,
                time_sig_num INTEGER NOT NULL,
                time_sig_denom INTEGER NOT NULL,
                ticks_per_beat INTEGER NOT NULL,
                loop_start INTEGER NOT NULL,
                loop_end INTEGER NOT NULL,
                looping INTEGER NOT NULL,
                key TEXT NOT NULL DEFAULT 'C',
                scale TEXT NOT NULL DEFAULT 'Major',
                tuning_a4 REAL NOT NULL DEFAULT 440.0,
                snap INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sampler_configs (
                strip_id INTEGER PRIMARY KEY,
                buffer_id INTEGER,
                loop_mode INTEGER NOT NULL,
                pitch_tracking INTEGER NOT NULL,
                next_slice_id INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sampler_slices (
                strip_id INTEGER NOT NULL,
                slice_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                start_pos REAL NOT NULL,
                end_pos REAL NOT NULL,
                name TEXT NOT NULL,
                root_note INTEGER NOT NULL,
                PRIMARY KEY (strip_id, slice_id)
            );

            CREATE TABLE IF NOT EXISTS automation_lanes (
                id INTEGER PRIMARY KEY,
                target_type TEXT NOT NULL,
                target_strip_id INTEGER NOT NULL,
                target_effect_idx INTEGER,
                target_param_idx INTEGER,
                enabled INTEGER NOT NULL,
                min_value REAL NOT NULL,
                max_value REAL NOT NULL
            );

            CREATE TABLE IF NOT EXISTS automation_points (
                lane_id INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                value REAL NOT NULL,
                curve_type TEXT NOT NULL,
                PRIMARY KEY (lane_id, tick)
            );

            -- Clear existing data
            DELETE FROM automation_points;
            DELETE FROM automation_lanes;
            DELETE FROM sampler_slices;
            DELETE FROM sampler_configs;
            DELETE FROM piano_roll_notes;
            DELETE FROM piano_roll_tracks;
            DELETE FROM musical_settings;
            DELETE FROM strip_modulations;
            DELETE FROM strip_sends;
            DELETE FROM strip_effect_params;
            DELETE FROM strip_effects;
            DELETE FROM strip_source_params;
            DELETE FROM strips;
            DELETE FROM mixer_buses;
            DELETE FROM mixer_master;
            DELETE FROM session;
            ",
        )?;

        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version, applied_at) VALUES (2, datetime('now'))",
            [],
        )?;

        conn.execute(
            "INSERT INTO session (id, name, created_at, modified_at, next_strip_id)
             VALUES (1, 'default', datetime('now'), datetime('now'), ?1)",
            [&self.next_id],
        )?;

        self.save_strips(&conn)?;
        self.save_source_params(&conn)?;
        self.save_effects(&conn)?;
        self.save_sends(&conn)?;
        self.save_modulations(&conn)?;
        self.save_mixer(&conn)?;
        self.save_piano_roll(&conn, session)?;
        self.save_sampler_configs(&conn)?;
        self.save_automation(&conn)?;

        Ok(())
    }

    fn save_strips(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut stmt = conn.prepare(
            "INSERT INTO strips (id, name, position, source_type, filter_type, filter_cutoff, filter_resonance,
             lfo_enabled, lfo_rate, lfo_depth, lfo_shape, lfo_target,
             amp_attack, amp_decay, amp_sustain, amp_release, polyphonic, has_track,
             level, pan, mute, solo, output_target)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        )?;
        for (pos, strip) in self.strips.iter().enumerate() {
            let source_str = strip.source.short_name();
            let (filter_type, filter_cutoff, filter_res): (Option<String>, Option<f64>, Option<f64>) =
                if let Some(ref f) = strip.filter {
                    (Some(format!("{:?}", f.filter_type).to_lowercase()), Some(f.cutoff.value as f64), Some(f.resonance.value as f64))
                } else {
                    (None, None, None)
                };
            let lfo_shape_str = match strip.lfo.shape {
                LfoShape::Sine => "sine",
                LfoShape::Square => "square",
                LfoShape::Saw => "saw",
                LfoShape::Triangle => "triangle",
            };
            let lfo_target_str = match strip.lfo.target {
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
            let output_str = match strip.output_target {
                OutputTarget::Master => "master".to_string(),
                OutputTarget::Bus(n) => format!("bus:{}", n),
            };
            stmt.execute(rusqlite::params![
                strip.id, strip.name, pos as i32, source_str,
                filter_type, filter_cutoff, filter_res,
                strip.lfo.enabled, strip.lfo.rate as f64, strip.lfo.depth as f64,
                lfo_shape_str, lfo_target_str,
                strip.amp_envelope.attack as f64, strip.amp_envelope.decay as f64,
                strip.amp_envelope.sustain as f64, strip.amp_envelope.release as f64,
                strip.polyphonic, strip.has_track,
                strip.level as f64, strip.pan as f64, strip.mute, strip.solo,
                output_str,
            ])?;
        }
        Ok(())
    }

    fn save_source_params(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut stmt = conn.prepare(
            "INSERT INTO strip_source_params (strip_id, param_name, param_value, param_min, param_max, param_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for strip in &self.strips {
            for param in &strip.source_params {
                let (value, param_type) = match &param.value {
                    ParamValue::Float(v) => (*v as f64, "float"),
                    ParamValue::Int(v) => (*v as f64, "int"),
                    ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, "bool"),
                };
                stmt.execute(rusqlite::params![
                    strip.id, param.name, value, param.min as f64, param.max as f64, param_type,
                ])?;
            }
        }
        Ok(())
    }

    fn save_effects(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut effect_stmt = conn.prepare(
            "INSERT INTO strip_effects (strip_id, position, effect_type, enabled)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut param_stmt = conn.prepare(
            "INSERT INTO strip_effect_params (strip_id, effect_position, param_name, param_value)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for strip in &self.strips {
            for (pos, effect) in strip.effects.iter().enumerate() {
                let type_str = format!("{:?}", effect.effect_type).to_lowercase();
                effect_stmt.execute(rusqlite::params![strip.id, pos as i32, type_str, effect.enabled])?;
                for param in &effect.params {
                    let value = match &param.value {
                        ParamValue::Float(v) => *v as f64,
                        ParamValue::Int(v) => *v as f64,
                        ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                    };
                    param_stmt.execute(rusqlite::params![strip.id, pos as i32, param.name, value])?;
                }
            }
        }
        Ok(())
    }

    fn save_sends(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut stmt = conn.prepare(
            "INSERT INTO strip_sends (strip_id, bus_id, level, enabled)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for strip in &self.strips {
            for send in &strip.sends {
                stmt.execute(rusqlite::params![strip.id, send.bus_id, send.level as f64, send.enabled])?;
            }
        }
        Ok(())
    }

    fn save_modulations(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut stmt = conn.prepare(
            "INSERT INTO strip_modulations (strip_id, target_param, mod_type,
             lfo_rate, lfo_depth, env_attack, env_decay, env_sustain, env_release,
             source_strip_id, source_param_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;

        for strip in &self.strips {
            if let Some(ref f) = strip.filter {
                if let Some(ref ms) = f.cutoff.mod_source {
                    insert_mod_source(&mut stmt, strip.id, "cutoff", ms)?;
                }
                if let Some(ref ms) = f.resonance.mod_source {
                    insert_mod_source(&mut stmt, strip.id, "resonance", ms)?;
                }
            }
        }
        Ok(())
    }

    fn save_mixer(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut stmt = conn.prepare(
            "INSERT INTO mixer_buses (id, name, level, pan, mute, solo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for bus in &self.buses {
            stmt.execute(rusqlite::params![bus.id, bus.name, bus.level as f64, bus.pan as f64, bus.mute, bus.solo])?;
        }

        conn.execute(
            "INSERT INTO mixer_master (id, level, mute) VALUES (1, ?1, ?2)",
            rusqlite::params![self.master_level as f64, self.master_mute],
        )?;
        Ok(())
    }

    fn save_sampler_configs(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut config_stmt = conn.prepare(
            "INSERT INTO sampler_configs (strip_id, buffer_id, loop_mode, pitch_tracking, next_slice_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut slice_stmt = conn.prepare(
            "INSERT INTO sampler_slices (strip_id, slice_id, position, start_pos, end_pos, name, root_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for strip in &self.strips {
            if let Some(ref config) = strip.sampler_config {
                config_stmt.execute(rusqlite::params![
                    strip.id,
                    config.buffer_id.map(|id| id as i32),
                    config.loop_mode,
                    config.pitch_tracking,
                    config.next_slice_id() as i32,
                ])?;

                for (pos, slice) in config.slices.iter().enumerate() {
                    slice_stmt.execute(rusqlite::params![
                        strip.id,
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

    fn save_automation(&self, conn: &SqlConnection) -> SqlResult<()> {
        let mut lane_stmt = conn.prepare(
            "INSERT INTO automation_lanes (id, target_type, target_strip_id, target_effect_idx, target_param_idx, enabled, min_value, max_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let mut point_stmt = conn.prepare(
            "INSERT INTO automation_points (lane_id, tick, value, curve_type)
             VALUES (?1, ?2, ?3, ?4)",
        )?;

        for lane in &self.automation.lanes {
            let (target_type, strip_id, effect_idx, param_idx) = match &lane.target {
                super::automation::AutomationTarget::StripLevel(id) => ("strip_level", *id, None, None),
                super::automation::AutomationTarget::StripPan(id) => ("strip_pan", *id, None, None),
                super::automation::AutomationTarget::FilterCutoff(id) => ("filter_cutoff", *id, None, None),
                super::automation::AutomationTarget::FilterResonance(id) => ("filter_resonance", *id, None, None),
                super::automation::AutomationTarget::EffectParam(id, fx, param) => {
                    ("effect_param", *id, Some(*fx as i32), Some(*param as i32))
                }
                super::automation::AutomationTarget::SamplerRate(id) => ("sampler_rate", *id, None, None),
                super::automation::AutomationTarget::SamplerAmp(id) => ("sampler_amp", *id, None, None),
            };

            lane_stmt.execute(rusqlite::params![
                lane.id as i32,
                target_type,
                strip_id,
                effect_idx,
                param_idx,
                lane.enabled,
                lane.min_value as f64,
                lane.max_value as f64,
            ])?;

            for point in &lane.points {
                let curve_str = match point.curve {
                    super::automation::CurveType::Linear => "linear",
                    super::automation::CurveType::Exponential => "exponential",
                    super::automation::CurveType::Step => "step",
                    super::automation::CurveType::SCurve => "scurve",
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

    fn save_piano_roll(&self, conn: &SqlConnection, session: &SessionState) -> SqlResult<()> {
        // Tracks
        {
            let mut stmt = conn.prepare(
                "INSERT INTO piano_roll_tracks (strip_id, position, polyphonic)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (pos, &sid) in self.piano_roll.track_order.iter().enumerate() {
                if let Some(track) = self.piano_roll.tracks.get(&sid) {
                    stmt.execute(rusqlite::params![sid, pos as i32, track.polyphonic])?;
                }
            }
        }

        // Notes
        {
            let mut stmt = conn.prepare(
                "INSERT INTO piano_roll_notes (track_strip_id, tick, duration, pitch, velocity)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for track in self.piano_roll.tracks.values() {
                for note in &track.notes {
                    stmt.execute(rusqlite::params![track.module_id, note.tick, note.duration, note.pitch, note.velocity])?;
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
                self.piano_roll.ticks_per_beat,
                self.piano_roll.loop_start,
                self.piano_roll.loop_end,
                self.piano_roll.looping,
                session.key.name(),
                session.scale.name(),
                session.tuning_a4 as f64,
                session.snap,
            ],
        )?;
        Ok(())
    }

    /// Load from SQLite
    pub fn load(path: &Path) -> SqlResult<(Self, SessionState)> {
        let conn = SqlConnection::open(path)?;

        let next_id: StripId = conn.query_row(
            "SELECT next_strip_id FROM session WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        let mut strips = load_strips(&conn)?;
        load_source_params(&conn, &mut strips)?;
        load_effects(&conn, &mut strips)?;
        load_sends(&conn, &mut strips)?;
        load_modulations(&conn, &mut strips)?;
        load_sampler_configs(&conn, &mut strips)?;
        let buses = load_buses(&conn)?;
        let (master_level, master_mute) = load_master(&conn);
        let (piano_roll, session) = load_piano_roll(&conn)?;
        let automation = load_automation(&conn)?;

        Ok((Self {
            strips,
            selected: None,
            next_id,
            buses,
            master_level,
            master_mute,
            piano_roll,
            mixer_selection: MixerSelection::default(),
            automation,
            midi_recording: super::midi_recording::MidiRecordingState::new(),
        }, session))
    }
}

// --- Load helpers ---

fn load_strips(conn: &SqlConnection) -> SqlResult<Vec<Strip>> {
    let mut strips = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT id, name, source_type, filter_type, filter_cutoff, filter_resonance,
         COALESCE(lfo_enabled, 0) as lfo_enabled,
         COALESCE(lfo_rate, 2.0) as lfo_rate,
         COALESCE(lfo_depth, 0.5) as lfo_depth,
         COALESCE(lfo_shape, 'sine') as lfo_shape,
         COALESCE(lfo_target, 'filter') as lfo_target,
         amp_attack, amp_decay, amp_sustain, amp_release, polyphonic, has_track,
         level, pan, mute, solo, output_target
         FROM strips ORDER BY position",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: StripId = row.get(0)?;
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
        let has_track: bool = row.get(16)?;
        let level: f64 = row.get(17)?;
        let pan: f64 = row.get(18)?;
        let mute: bool = row.get(19)?;
        let solo: bool = row.get(20)?;
        let output_str: String = row.get(21)?;
        Ok((id, name, source_str, filter_type_str, filter_cutoff, filter_res,
            lfo_enabled, lfo_rate, lfo_depth, lfo_shape_str, lfo_target_str,
            attack, decay, sustain, release, polyphonic, has_track,
            level, pan, mute, solo, output_str))
    })?;

    for result in rows {
        let (id, name, source_str, filter_type_str, filter_cutoff, filter_res,
             lfo_enabled, lfo_rate, lfo_depth, lfo_shape_str, lfo_target_str,
             attack, decay, sustain, release, polyphonic, has_track,
             level, pan, mute, solo, output_str) = result?;

        let source = parse_osc_type(&source_str);
        let filter = filter_type_str.map(|ft| {
            let filter_type = parse_filter_type(&ft);
            let mut config = FilterConfig::new(filter_type);
            if let Some(c) = filter_cutoff { config.cutoff.value = c as f32; }
            if let Some(r) = filter_res { config.resonance.value = r as f32; }
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
            n.parse::<u8>().map(OutputTarget::Bus).unwrap_or(OutputTarget::Master)
        } else {
            OutputTarget::Master
        };

        let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();
        let sampler_config = if source.is_sampler() {
            Some(super::sampler::SamplerConfig::default())
        } else {
            None
        };

        strips.push(Strip {
            id,
            name,
            source,
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
            has_track,
            level: level as f32,
            pan: pan as f32,
            mute,
            solo,
            output_target,
            sends,
            sampler_config,
        });
    }
    Ok(strips)
}

fn load_source_params(conn: &SqlConnection, strips: &mut [Strip]) -> SqlResult<()> {
    let mut stmt = conn.prepare(
        "SELECT param_name, param_value, param_min, param_max, param_type
         FROM strip_source_params WHERE strip_id = ?1",
    )?;
    for strip in strips {
        let params: Vec<Param> = stmt.query_map([&strip.id], |row| {
            let name: String = row.get(0)?;
            let value: f64 = row.get(1)?;
            let min: f64 = row.get(2)?;
            let max: f64 = row.get(3)?;
            let param_type: String = row.get(4)?;
            Ok((name, value, min, max, param_type))
        })?.filter_map(|r| r.ok())
        .map(|(name, value, min, max, param_type)| {
            let pv = match param_type.as_str() {
                "int" => ParamValue::Int(value as i32),
                "bool" => ParamValue::Bool(value != 0.0),
                _ => ParamValue::Float(value as f32),
            };
            Param { name, value: pv, min: min as f32, max: max as f32 }
        }).collect();
        if !params.is_empty() {
            strip.source_params = params;
        }
    }
    Ok(())
}

fn load_effects(conn: &SqlConnection, strips: &mut [Strip]) -> SqlResult<()> {
    let mut effect_stmt = conn.prepare(
        "SELECT position, effect_type, enabled FROM strip_effects WHERE strip_id = ?1 ORDER BY position",
    )?;
    let mut param_stmt = conn.prepare(
        "SELECT param_name, param_value FROM strip_effect_params WHERE strip_id = ?1 AND effect_position = ?2",
    )?;
    for strip in strips {
        let effects: Vec<(i32, String, bool)> = effect_stmt.query_map([&strip.id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.filter_map(|r| r.ok()).collect();

        for (pos, type_str, enabled) in effects {
            let effect_type = parse_effect_type(&type_str);
            let mut slot = EffectSlot::new(effect_type);
            slot.enabled = enabled;

            let params: Vec<(String, f64)> = param_stmt.query_map(rusqlite::params![strip.id, pos], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.filter_map(|r| r.ok()).collect();

            for (name, value) in params {
                if let Some(p) = slot.params.iter_mut().find(|p| p.name == name) {
                    p.value = ParamValue::Float(value as f32);
                }
            }

            strip.effects.push(slot);
        }
    }
    Ok(())
}

fn load_sends(conn: &SqlConnection, strips: &mut [Strip]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT strip_id, bus_id, level, enabled FROM strip_sends",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let strip_id: StripId = row.get(0)?;
            let bus_id: u8 = row.get(1)?;
            let level: f64 = row.get(2)?;
            let enabled: bool = row.get(3)?;
            Ok((strip_id, bus_id, level, enabled))
        }) {
            for result in rows {
                if let Ok((strip_id, bus_id, level, enabled)) = result {
                    if let Some(strip) = strips.iter_mut().find(|s| s.id == strip_id) {
                        if let Some(send) = strip.sends.iter_mut().find(|s| s.bus_id == bus_id) {
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

fn insert_mod_source(
    stmt: &mut rusqlite::Statement,
    strip_id: StripId,
    target: &str,
    ms: &ModSource,
) -> SqlResult<()> {
    match ms {
        ModSource::Lfo(lfo) => stmt.execute(rusqlite::params![
            strip_id, target, "lfo",
            lfo.rate as f64, lfo.depth as f64,
            None::<f64>, None::<f64>, None::<f64>, None::<f64>,
            None::<i32>, None::<String>
        ]),
        ModSource::Envelope(env) => stmt.execute(rusqlite::params![
            strip_id, target, "envelope",
            None::<f64>, None::<f64>,
            env.attack as f64, env.decay as f64, env.sustain as f64, env.release as f64,
            None::<i32>, None::<String>
        ]),
        ModSource::StripParam(sid, name) => stmt.execute(rusqlite::params![
            strip_id, target, "strip_param",
            None::<f64>, None::<f64>,
            None::<f64>, None::<f64>, None::<f64>, None::<f64>,
            *sid, name
        ]),
    }?;
    Ok(())
}

fn load_modulations(conn: &SqlConnection, strips: &mut [Strip]) -> SqlResult<()> {
    if let Ok(mut stmt) = conn.prepare(
        "SELECT strip_id, target_param, mod_type, lfo_rate, lfo_depth,
         env_attack, env_decay, env_sustain, env_release,
         source_strip_id, source_param_name
         FROM strip_modulations",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, StripId>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<f64>>(4)?,
                row.get::<_, Option<f64>>(5)?,
                row.get::<_, Option<f64>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<f64>>(8)?,
                row.get::<_, Option<StripId>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        }) {
            for result in rows {
                if let Ok((strip_id, target, mod_type, lfo_rate, lfo_depth,
                           env_a, env_d, env_s, env_r, src_id, src_name)) = result {
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
                        "strip_param" => src_id.zip(src_name).map(|(id, name)| ModSource::StripParam(id, name)),
                        _ => None,
                    };

                    if let Some(ms) = mod_source {
                        if let Some(strip) = strips.iter_mut().find(|s| s.id == strip_id) {
                            if let Some(ref mut f) = strip.filter {
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

fn load_buses(conn: &SqlConnection) -> SqlResult<Vec<MixerBus>> {
    let mut buses: Vec<MixerBus> = (1..=MAX_BUSES as u8).map(MixerBus::new).collect();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, name, level, pan, mute, solo FROM mixer_buses ORDER BY id",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, u8>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?, row.get::<_, bool>(4)?, row.get::<_, bool>(5)?))
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

fn load_master(conn: &SqlConnection) -> (f32, bool) {
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

fn load_piano_roll(conn: &SqlConnection) -> SqlResult<(PianoRollState, SessionState)> {
    let mut piano_roll = PianoRollState::new();
    let mut session = SessionState::default();

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
        session.bpm = row.0 as u16;
        session.time_signature = (row.1, row.2);
        session.key = parse_key(&row.7);
        session.scale = parse_scale(&row.8);
        session.tuning_a4 = row.9 as f32;
        session.snap = row.10;
        piano_roll.bpm = row.0 as f32;
        piano_roll.time_signature = (row.1, row.2);
        piano_roll.ticks_per_beat = row.3;
        piano_roll.loop_start = row.4;
        piano_roll.loop_end = row.5;
        piano_roll.looping = row.6;
    }

    // Load tracks
    if let Ok(mut stmt) = conn.prepare(
        "SELECT strip_id, polyphonic FROM piano_roll_tracks ORDER BY position",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, StripId>(0)?, row.get::<_, bool>(1)?))
        }) {
            for result in rows {
                if let Ok((strip_id, polyphonic)) = result {
                    piano_roll.track_order.push(strip_id);
                    piano_roll.tracks.insert(
                        strip_id,
                        super::piano_roll::Track {
                            module_id: strip_id,
                            notes: Vec::new(),
                            polyphonic,
                        },
                    );
                }
            }
        }
    }

    // Load notes
    if let Ok(mut stmt) = conn.prepare(
        "SELECT track_strip_id, tick, duration, pitch, velocity FROM piano_roll_notes",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, StripId>(0)?, row.get::<_, u32>(1)?, row.get::<_, u32>(2)?,
                row.get::<_, u8>(3)?, row.get::<_, u8>(4)?))
        }) {
            for result in rows {
                if let Ok((strip_id, tick, duration, pitch, velocity)) = result {
                    if let Some(track) = piano_roll.tracks.get_mut(&strip_id) {
                        track.notes.push(super::piano_roll::Note { tick, duration, pitch, velocity });
                    }
                }
            }
        }
    }

    Ok((piano_roll, session))
}

fn load_sampler_configs(conn: &SqlConnection, strips: &mut [Strip]) -> SqlResult<()> {
    // Load sampler configs
    if let Ok(mut config_stmt) = conn.prepare(
        "SELECT strip_id, buffer_id, loop_mode, pitch_tracking, next_slice_id FROM sampler_configs",
    ) {
        if let Ok(rows) = config_stmt.query_map([], |row| {
            Ok((
                row.get::<_, StripId>(0)?,
                row.get::<_, Option<i32>>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, i32>(4)?,
            ))
        }) {
            for result in rows {
                if let Ok((strip_id, buffer_id, loop_mode, pitch_tracking, next_slice_id)) = result {
                    if let Some(strip) = strips.iter_mut().find(|s| s.id == strip_id) {
                        if let Some(ref mut config) = strip.sampler_config {
                            config.buffer_id = buffer_id.map(|id| id as super::sampler::BufferId);
                            config.loop_mode = loop_mode;
                            config.pitch_tracking = pitch_tracking;
                            config.set_next_slice_id(next_slice_id as super::sampler::SliceId);
                            // Clear default slices - we'll load them from the database
                            config.slices.clear();
                        }
                    }
                }
            }
        }
    }

    // Load slices
    if let Ok(mut slice_stmt) = conn.prepare(
        "SELECT strip_id, slice_id, start_pos, end_pos, name, root_note FROM sampler_slices ORDER BY strip_id, position",
    ) {
        if let Ok(rows) = slice_stmt.query_map([], |row| {
            Ok((
                row.get::<_, StripId>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)?,
            ))
        }) {
            for result in rows {
                if let Ok((strip_id, slice_id, start, end, name, root_note)) = result {
                    if let Some(strip) = strips.iter_mut().find(|s| s.id == strip_id) {
                        if let Some(ref mut config) = strip.sampler_config {
                            config.slices.push(super::sampler::Slice {
                                id: slice_id as super::sampler::SliceId,
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

fn load_automation(conn: &SqlConnection) -> SqlResult<super::automation::AutomationState> {
    use super::automation::{AutomationLane, AutomationPoint, AutomationState, AutomationTarget, CurveType};

    let mut state = AutomationState::new();

    // Load lanes
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, target_type, target_strip_id, target_effect_idx, target_param_idx, enabled, min_value, max_value
         FROM automation_lanes",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, StripId>(2)?,
                row.get::<_, Option<i32>>(3)?,
                row.get::<_, Option<i32>>(4)?,
                row.get::<_, bool>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
            ))
        }) {
            for result in rows {
                if let Ok((id, target_type, strip_id, effect_idx, param_idx, enabled, min_value, max_value)) = result {
                    let target = match target_type.as_str() {
                        "strip_level" => AutomationTarget::StripLevel(strip_id),
                        "strip_pan" => AutomationTarget::StripPan(strip_id),
                        "filter_cutoff" => AutomationTarget::FilterCutoff(strip_id),
                        "filter_resonance" => AutomationTarget::FilterResonance(strip_id),
                        "effect_param" => {
                            let fx = effect_idx.unwrap_or(0) as usize;
                            let param = param_idx.unwrap_or(0) as usize;
                            AutomationTarget::EffectParam(strip_id, fx, param)
                        }
                        "sampler_rate" => AutomationTarget::SamplerRate(strip_id),
                        "sampler_amp" => AutomationTarget::SamplerAmp(strip_id),
                        _ => continue,
                    };

                    let mut lane = AutomationLane::new(id as u32, target);
                    lane.enabled = enabled;
                    lane.min_value = min_value as f32;
                    lane.max_value = max_value as f32;
                    state.lanes.push(lane);
                }
            }
        }
    }

    // Load points
    if let Ok(mut stmt) = conn.prepare(
        "SELECT lane_id, tick, value, curve_type FROM automation_points ORDER BY lane_id, tick",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
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

    // Set selected lane if we have any
    if !state.lanes.is_empty() {
        state.selected_lane = Some(0);
    }

    Ok(state)
}

// --- Parse helpers ---

fn parse_key(s: &str) -> Key {
    Key::ALL.iter().find(|k| k.name() == s).copied().unwrap_or(Key::C)
}

fn parse_scale(s: &str) -> Scale {
    Scale::ALL.iter().find(|sc| sc.name() == s).copied().unwrap_or(Scale::Major)
}

fn parse_osc_type(s: &str) -> OscType {
    match s {
        "saw" => OscType::Saw,
        "sin" => OscType::Sin,
        "sqr" => OscType::Sqr,
        "tri" => OscType::Tri,
        "audio_in" => OscType::AudioIn,
        "sampler" => OscType::Sampler,
        _ => OscType::Saw,
    }
}

fn parse_filter_type(s: &str) -> FilterType {
    match s {
        "lpf" => FilterType::Lpf,
        "hpf" => FilterType::Hpf,
        "bpf" => FilterType::Bpf,
        _ => FilterType::Lpf,
    }
}

fn parse_effect_type(s: &str) -> EffectType {
    match s {
        "delay" => EffectType::Delay,
        "reverb" => EffectType::Reverb,
        _ => EffectType::Delay,
    }
}
