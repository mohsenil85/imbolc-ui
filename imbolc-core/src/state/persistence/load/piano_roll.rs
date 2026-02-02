use rusqlite::{Connection as SqlConnection, Result as SqlResult};

use crate::state::instrument::*;
use crate::state::piano_roll::PianoRollState;
use crate::state::persistence::conversion::{parse_key, parse_scale};

/// Musical settings loaded from the database, used to populate SessionState fields.
pub(crate) struct MusicalSettingsLoaded {
    pub bpm: u16,
    pub time_signature: (u8, u8),
    pub key: crate::state::music::Key,
    pub scale: crate::state::music::Scale,
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
            key: crate::state::music::Key::C,
            scale: crate::state::music::Scale::Major,
            tuning_a4: 440.0,
            snap: false,
            humanize_velocity: 0.0,
            humanize_timing: 0.0,
        }
    }
}

pub(crate) fn load_piano_roll(conn: &SqlConnection) -> SqlResult<(PianoRollState, MusicalSettingsLoaded)> {
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
                        crate::state::piano_roll::Track {
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
                        track.notes.push(crate::state::piano_roll::Note { tick, duration, pitch, velocity, probability });
                    }
                }
            }
        }
    }

    // Ensure notes are sorted by tick (required for playback optimization)
    for track in piano_roll.tracks.values_mut() {
        track.notes.sort_by_key(|n| n.tick);
    }

    Ok((piano_roll, musical))
}

pub(crate) fn load_sampler_configs(conn: &SqlConnection, instruments: &mut [Instrument]) -> SqlResult<()> {
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
                            config.buffer_id = buffer_id.map(|id| id as crate::state::sampler::BufferId);
                            config.sample_name = sample_name;
                            config.loop_mode = loop_mode;
                            config.pitch_tracking = pitch_tracking;
                            config.set_next_slice_id(next_slice_id as crate::state::sampler::SliceId);
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
                            config.slices.push(crate::state::sampler::Slice {
                                id: slice_id as crate::state::sampler::SliceId,
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
