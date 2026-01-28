use std::path::Path;

use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use serde::{Deserialize, Serialize};

use super::music::{Key, Scale};
use super::param::{Param, ParamValue};
use super::piano_roll::PianoRollState;
use super::strip::*;

use crate::ui::frame::SessionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixerSelection {
    Strip(usize),  // index into strips vec
    Bus(u8),       // 1-8
    Master,
}

impl Default for MixerSelection {
    fn default() -> Self {
        Self::Strip(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripState {
    pub strips: Vec<Strip>,
    pub selected: Option<usize>,
    pub next_id: StripId,
    pub buses: Vec<MixerBus>,
    pub master_level: f32,
    pub master_mute: bool,
    pub piano_roll: PianoRollState,
    pub mixer_selection: MixerSelection,
}

impl StripState {
    pub fn new() -> Self {
        let buses = (1..=MAX_BUSES as u8).map(MixerBus::new).collect();
        Self {
            strips: Vec::new(),
            selected: None,
            next_id: 0,
            buses,
            master_level: 1.0,
            master_mute: false,
            piano_roll: PianoRollState::new(),
            mixer_selection: MixerSelection::default(),
        }
    }

    pub fn add_strip(&mut self, source: OscType) -> StripId {
        let id = self.next_id;
        self.next_id += 1;
        let strip = Strip::new(id, source);

        // Auto-add piano roll track if strip has_track
        if strip.has_track {
            self.piano_roll.add_track(id);
        }

        self.strips.push(strip);

        if self.selected.is_none() {
            self.selected = Some(0);
        }

        id
    }

    pub fn remove_strip(&mut self, id: StripId) {
        if let Some(pos) = self.strips.iter().position(|s| s.id == id) {
            self.strips.remove(pos);
            self.piano_roll.remove_track(id);

            if let Some(sel) = self.selected {
                if sel >= self.strips.len() {
                    self.selected = if self.strips.is_empty() {
                        None
                    } else {
                        Some(self.strips.len() - 1)
                    };
                }
            }
        }
    }

    pub fn strip(&self, id: StripId) -> Option<&Strip> {
        self.strips.iter().find(|s| s.id == id)
    }

    pub fn strip_mut(&mut self, id: StripId) -> Option<&mut Strip> {
        self.strips.iter_mut().find(|s| s.id == id)
    }

    pub fn selected_strip(&self) -> Option<&Strip> {
        self.selected.and_then(|idx| self.strips.get(idx))
    }

    pub fn selected_strip_mut(&mut self) -> Option<&mut Strip> {
        self.selected.and_then(|idx| self.strips.get_mut(idx))
    }

    pub fn select_next(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(idx) if idx < self.strips.len() - 1 => Some(idx + 1),
            Some(idx) => Some(idx),
        };
    }

    pub fn select_prev(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(0) => Some(0),
            Some(idx) => Some(idx - 1),
        };
    }

    pub fn bus(&self, id: u8) -> Option<&MixerBus> {
        self.buses.get((id - 1) as usize)
    }

    pub fn bus_mut(&mut self, id: u8) -> Option<&mut MixerBus> {
        self.buses.get_mut((id - 1) as usize)
    }

    /// Check if any strip is soloed
    pub fn any_strip_solo(&self) -> bool {
        self.strips.iter().any(|s| s.solo)
    }

    /// Check if any bus is soloed
    pub fn any_bus_solo(&self) -> bool {
        self.buses.iter().any(|b| b.solo)
    }

    /// Compute effective mute for a strip, considering solo state
    pub fn effective_strip_mute(&self, strip: &Strip) -> bool {
        if self.any_strip_solo() {
            !strip.solo
        } else {
            strip.mute || self.master_mute
        }
    }

    /// Compute effective mute for a bus, considering solo state
    pub fn effective_bus_mute(&self, bus: &MixerBus) -> bool {
        if self.any_bus_solo() {
            !bus.solo
        } else {
            bus.mute
        }
    }

    /// Collect mixer updates for all strips (strip_id, level, mute)
    pub fn collect_strip_updates(&self) -> Vec<(StripId, f32, bool)> {
        self.strips
            .iter()
            .map(|s| (s.id, s.level * self.master_level, self.effective_strip_mute(s)))
            .collect()
    }

    /// Move mixer selection left/right
    pub fn mixer_move(&mut self, delta: i8) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(idx) => {
                let new_idx = (idx as i32 + delta as i32).clamp(0, self.strips.len().saturating_sub(1) as i32) as usize;
                MixerSelection::Strip(new_idx)
            }
            MixerSelection::Bus(id) => {
                let new_id = (id as i8 + delta).clamp(1, MAX_BUSES as i8) as u8;
                MixerSelection::Bus(new_id)
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Jump to first (1) or last (-1) in current section
    pub fn mixer_jump(&mut self, direction: i8) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(_) => {
                if direction > 0 {
                    MixerSelection::Strip(0)
                } else {
                    MixerSelection::Strip(self.strips.len().saturating_sub(1))
                }
            }
            MixerSelection::Bus(_) => {
                if direction > 0 {
                    MixerSelection::Bus(1)
                } else {
                    MixerSelection::Bus(MAX_BUSES as u8)
                }
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Cycle between strip/bus/master sections
    pub fn mixer_cycle_section(&mut self) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(_) => MixerSelection::Bus(1),
            MixerSelection::Bus(_) => MixerSelection::Master,
            MixerSelection::Master => MixerSelection::Strip(0),
        };
    }

    /// Cycle output target for the selected strip
    pub fn mixer_cycle_output(&mut self) {
        if let MixerSelection::Strip(idx) = self.mixer_selection {
            if let Some(strip) = self.strips.get_mut(idx) {
                strip.output_target = match strip.output_target {
                    OutputTarget::Master => OutputTarget::Bus(1),
                    OutputTarget::Bus(n) if n < MAX_BUSES as u8 => OutputTarget::Bus(n + 1),
                    OutputTarget::Bus(_) => OutputTarget::Master,
                };
            }
        }
    }

    /// Cycle output target backwards for the selected strip
    pub fn mixer_cycle_output_reverse(&mut self) {
        if let MixerSelection::Strip(idx) = self.mixer_selection {
            if let Some(strip) = self.strips.get_mut(idx) {
                strip.output_target = match strip.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }

    /// Strips that have tracks (for piano roll)
    pub fn strips_with_tracks(&self) -> Vec<&Strip> {
        self.strips.iter().filter(|s| s.has_track).collect()
    }

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

            -- Clear existing data
            DELETE FROM piano_roll_notes;
            DELETE FROM piano_roll_tracks;
            DELETE FROM musical_settings;
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

        // Insert strips
        {
            let mut stmt = conn.prepare(
                "INSERT INTO strips (id, name, position, source_type, filter_type, filter_cutoff, filter_resonance,
                 amp_attack, amp_decay, amp_sustain, amp_release, polyphonic, has_track,
                 level, pan, mute, solo, output_target)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            )?;
            for (pos, strip) in self.strips.iter().enumerate() {
                let source_str = strip.source.short_name();
                let (filter_type, filter_cutoff, filter_res): (Option<String>, Option<f64>, Option<f64>) =
                    if let Some(ref f) = strip.filter {
                        (Some(format!("{:?}", f.filter_type).to_lowercase()), Some(f.cutoff.value as f64), Some(f.resonance.value as f64))
                    } else {
                        (None, None, None)
                    };
                let output_str = match strip.output_target {
                    OutputTarget::Master => "master".to_string(),
                    OutputTarget::Bus(n) => format!("bus:{}", n),
                };
                stmt.execute(rusqlite::params![
                    strip.id, strip.name, pos as i32, source_str,
                    filter_type, filter_cutoff, filter_res,
                    strip.amp_envelope.attack as f64, strip.amp_envelope.decay as f64,
                    strip.amp_envelope.sustain as f64, strip.amp_envelope.release as f64,
                    strip.polyphonic, strip.has_track,
                    strip.level as f64, strip.pan as f64, strip.mute, strip.solo,
                    output_str,
                ])?;
            }
        }

        // Insert source params
        {
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
        }

        // Insert effects
        {
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
        }

        // Insert sends
        {
            let mut stmt = conn.prepare(
                "INSERT INTO strip_sends (strip_id, bus_id, level, enabled)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for strip in &self.strips {
                for send in &strip.sends {
                    stmt.execute(rusqlite::params![strip.id, send.bus_id, send.level as f64, send.enabled])?;
                }
            }
        }

        // Insert mixer buses
        {
            let mut stmt = conn.prepare(
                "INSERT INTO mixer_buses (id, name, level, pan, mute, solo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for bus in &self.buses {
                stmt.execute(rusqlite::params![bus.id, bus.name, bus.level as f64, bus.pan as f64, bus.mute, bus.solo])?;
            }
        }

        // Insert mixer master
        conn.execute(
            "INSERT INTO mixer_master (id, level, mute) VALUES (1, ?1, ?2)",
            rusqlite::params![self.master_level as f64, self.master_mute],
        )?;

        // Insert piano roll tracks
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

        // Insert piano roll notes
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

        // Insert musical settings
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

        // Load strips
        let mut strips = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT id, name, source_type, filter_type, filter_cutoff, filter_resonance,
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
                let attack: f64 = row.get(6)?;
                let decay: f64 = row.get(7)?;
                let sustain: f64 = row.get(8)?;
                let release: f64 = row.get(9)?;
                let polyphonic: bool = row.get(10)?;
                let has_track: bool = row.get(11)?;
                let level: f64 = row.get(12)?;
                let pan: f64 = row.get(13)?;
                let mute: bool = row.get(14)?;
                let solo: bool = row.get(15)?;
                let output_str: String = row.get(16)?;
                Ok((id, name, source_str, filter_type_str, filter_cutoff, filter_res,
                    attack, decay, sustain, release, polyphonic, has_track,
                    level, pan, mute, solo, output_str))
            })?;

            for result in rows {
                let (id, name, source_str, filter_type_str, filter_cutoff, filter_res,
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
                let output_target = if output_str == "master" {
                    OutputTarget::Master
                } else if let Some(n) = output_str.strip_prefix("bus:") {
                    n.parse::<u8>().map(OutputTarget::Bus).unwrap_or(OutputTarget::Master)
                } else {
                    OutputTarget::Master
                };

                let sends = (1..=MAX_BUSES as u8).map(MixerSend::new).collect();

                strips.push(Strip {
                    id,
                    name,
                    source,
                    source_params: OscType::default_params(), // overwritten below
                    filter,
                    effects: Vec::new(), // loaded below
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
                });
            }
        }

        // Load source params
        {
            let mut stmt = conn.prepare(
                "SELECT param_name, param_value, param_min, param_max, param_type
                 FROM strip_source_params WHERE strip_id = ?1",
            )?;
            for strip in &mut strips {
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
        }

        // Load effects
        {
            let mut effect_stmt = conn.prepare(
                "SELECT position, effect_type, enabled FROM strip_effects WHERE strip_id = ?1 ORDER BY position",
            )?;
            let mut param_stmt = conn.prepare(
                "SELECT param_name, param_value FROM strip_effect_params WHERE strip_id = ?1 AND effect_position = ?2",
            )?;
            for strip in &mut strips {
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
        }

        // Load sends
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

        // Load mixer buses
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

        // Load mixer master
        let mut master_level = 1.0f32;
        let mut master_mute = false;
        if let Ok(row) = conn.query_row(
            "SELECT level, mute FROM mixer_master WHERE id = 1",
            [],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, bool>(1)?)),
        ) {
            master_level = row.0 as f32;
            master_mute = row.1;
        }

        // Load piano roll
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

        // Load piano roll tracks
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

        // Load piano roll notes
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

        Ok((Self {
            strips,
            selected: None,
            next_id,
            buses,
            master_level,
            master_mute,
            piano_roll,
            mixer_selection: MixerSelection::default(),
        }, session))
    }
}

impl Default for StripState {
    fn default() -> Self {
        Self::new()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_state_creation() {
        let state = StripState::new();
        assert_eq!(state.strips.len(), 0);
        assert_eq!(state.selected, None);
        assert_eq!(state.buses.len(), MAX_BUSES);
    }

    #[test]
    fn test_add_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
        assert_eq!(state.strips[1].id, id2);
        assert_eq!(state.selected, Some(0));
        // Piano roll tracks auto-created
        assert_eq!(state.piano_roll.track_order.len(), 2);
    }

    #[test]
    fn test_remove_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);
        let _id3 = state.add_strip(OscType::Sqr);

        state.remove_strip(id2);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
        assert_eq!(state.piano_roll.track_order.len(), 2);
    }

    #[test]
    fn test_remove_last_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        state.selected = Some(1);
        state.remove_strip(id2);

        assert_eq!(state.selected, Some(0));
        assert_eq!(state.strips[0].id, id1);
    }

    #[test]
    fn test_remove_all_strips() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);

        state.remove_strip(id1);
        assert_eq!(state.selected, None);
        assert!(state.strips.is_empty());
    }

    #[test]
    fn test_select_navigation() {
        let mut state = StripState::new();
        state.add_strip(OscType::Saw);
        state.add_strip(OscType::Sin);
        state.add_strip(OscType::Sqr);

        assert_eq!(state.selected, Some(0));
        state.select_next();
        assert_eq!(state.selected, Some(1));
        state.select_next();
        assert_eq!(state.selected, Some(2));
        state.select_next();
        assert_eq!(state.selected, Some(2)); // stay at end
        state.select_prev();
        assert_eq!(state.selected, Some(1));
        state.select_prev();
        assert_eq!(state.selected, Some(0));
        state.select_prev();
        assert_eq!(state.selected, Some(0)); // stay at start
    }

    #[test]
    fn test_mixer_selection() {
        let mut state = StripState::new();
        state.add_strip(OscType::Saw);
        state.add_strip(OscType::Sin);

        state.mixer_selection = MixerSelection::Strip(0);
        state.mixer_move(1);
        assert_eq!(state.mixer_selection, MixerSelection::Strip(1));

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Bus(1));

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Master);

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Strip(0));
    }

    #[test]
    fn test_save_and_load() {
        use tempfile::tempdir;

        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let _id2 = state.add_strip(OscType::Sin);

        // Add a filter to first strip
        if let Some(strip) = state.strip_mut(id1) {
            strip.filter = Some(FilterConfig::new(FilterType::Lpf));
            strip.effects.push(EffectSlot::new(EffectType::Reverb));
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");
        assert_eq!(loaded.strips.len(), 2);
        assert_eq!(loaded.strips[0].source, OscType::Saw);
        assert_eq!(loaded.strips[1].source, OscType::Sin);
        assert!(loaded.strips[0].filter.is_some());
        assert_eq!(loaded.strips[0].effects.len(), 1);
        assert_eq!(loaded.next_id, 2);
    }
}
