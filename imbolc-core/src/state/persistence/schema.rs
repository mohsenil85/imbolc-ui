use rusqlite::{Connection as SqlConnection, Result as SqlResult};

/// Create all tables and clear existing data for a fresh save
pub(super) fn create_tables_and_clear(conn: &SqlConnection) -> SqlResult<()> {
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
                next_instrument_id INTEGER NOT NULL,
                selected_instrument INTEGER,
                selected_automation_lane INTEGER
            );

            CREATE TABLE IF NOT EXISTS instruments (
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
                level REAL NOT NULL,
                pan REAL NOT NULL,
                mute INTEGER NOT NULL,
                solo INTEGER NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                output_target TEXT NOT NULL,
                vst_state_path TEXT,
                arp_enabled INTEGER NOT NULL DEFAULT 0,
                arp_direction TEXT NOT NULL DEFAULT 'up',
                arp_rate TEXT NOT NULL DEFAULT '1/8',
                arp_octaves INTEGER NOT NULL DEFAULT 1,
                arp_gate REAL NOT NULL DEFAULT 0.5,
                chord_shape TEXT,
                convolution_ir_path TEXT,
                eq_enabled INTEGER,
                layer_group INTEGER
            );

            CREATE TABLE IF NOT EXISTS instrument_eq_bands (
                instrument_id INTEGER NOT NULL,
                band_index INTEGER NOT NULL,
                band_type TEXT NOT NULL,
                freq REAL NOT NULL,
                gain REAL NOT NULL,
                q REAL NOT NULL,
                enabled INTEGER NOT NULL,
                PRIMARY KEY (instrument_id, band_index)
            );

            CREATE TABLE IF NOT EXISTS instrument_source_params (
                instrument_id INTEGER NOT NULL,
                param_name TEXT NOT NULL,
                param_value REAL NOT NULL,
                param_min REAL NOT NULL,
                param_max REAL NOT NULL,
                param_type TEXT NOT NULL,
                PRIMARY KEY (instrument_id, param_name)
            );

            CREATE TABLE IF NOT EXISTS instrument_effects (
                instrument_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                effect_type TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                vst_state_path TEXT,
                PRIMARY KEY (instrument_id, position)
            );

            CREATE TABLE IF NOT EXISTS instrument_effect_params (
                instrument_id INTEGER NOT NULL,
                effect_position INTEGER NOT NULL,
                param_name TEXT NOT NULL,
                param_value REAL NOT NULL,
                PRIMARY KEY (instrument_id, effect_position, param_name)
            );

            CREATE TABLE IF NOT EXISTS instrument_sends (
                instrument_id INTEGER NOT NULL,
                bus_id INTEGER NOT NULL,
                level REAL NOT NULL,
                enabled INTEGER NOT NULL,
                PRIMARY KEY (instrument_id, bus_id)
            );

            CREATE TABLE IF NOT EXISTS instrument_modulations (
                instrument_id INTEGER NOT NULL,
                target_param TEXT NOT NULL,
                mod_type TEXT NOT NULL,
                lfo_rate REAL,
                lfo_depth REAL,
                env_attack REAL,
                env_decay REAL,
                env_sustain REAL,
                env_release REAL,
                source_instrument_id INTEGER,
                source_param_name TEXT,
                PRIMARY KEY (instrument_id, target_param)
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
                instrument_id INTEGER PRIMARY KEY,
                position INTEGER NOT NULL,
                polyphonic INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS piano_roll_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_instrument_id INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                duration INTEGER NOT NULL,
                pitch INTEGER NOT NULL,
                velocity INTEGER NOT NULL,
                probability REAL NOT NULL DEFAULT 1.0
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
                snap INTEGER NOT NULL DEFAULT 0,
                swing_amount REAL NOT NULL DEFAULT 0.0,
                humanize_velocity REAL NOT NULL DEFAULT 0.0,
                humanize_timing REAL NOT NULL DEFAULT 0.0
            );

            CREATE TABLE IF NOT EXISTS sampler_configs (
                instrument_id INTEGER PRIMARY KEY,
                buffer_id INTEGER,
                sample_name TEXT,
                loop_mode INTEGER NOT NULL,
                pitch_tracking INTEGER NOT NULL,
                next_slice_id INTEGER NOT NULL,
                selected_slice INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS sampler_slices (
                instrument_id INTEGER NOT NULL,
                slice_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                start_pos REAL NOT NULL,
                end_pos REAL NOT NULL,
                name TEXT NOT NULL,
                root_note INTEGER NOT NULL,
                PRIMARY KEY (instrument_id, slice_id)
            );

            CREATE TABLE IF NOT EXISTS automation_lanes (
                id INTEGER PRIMARY KEY,
                target_type TEXT NOT NULL,
                target_instrument_id INTEGER NOT NULL,
                target_effect_idx INTEGER,
                target_param_idx INTEGER,
                enabled INTEGER NOT NULL,
                record_armed INTEGER NOT NULL DEFAULT 0,
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

            CREATE TABLE IF NOT EXISTS custom_synthdefs (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                synthdef_name TEXT NOT NULL,
                source_path TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS custom_synthdef_params (
                synthdef_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                name TEXT NOT NULL,
                default_val REAL NOT NULL,
                min_val REAL NOT NULL,
                max_val REAL NOT NULL,
                PRIMARY KEY (synthdef_id, position),
                FOREIGN KEY (synthdef_id) REFERENCES custom_synthdefs(id)
            );

            CREATE TABLE IF NOT EXISTS instrument_vst_params (
                instrument_id INTEGER NOT NULL,
                param_index INTEGER NOT NULL,
                value REAL NOT NULL,
                PRIMARY KEY (instrument_id, param_index)
            );

            CREATE TABLE IF NOT EXISTS effect_vst_params (
                instrument_id INTEGER NOT NULL,
                effect_position INTEGER NOT NULL,
                param_index INTEGER NOT NULL,
                value REAL NOT NULL,
                PRIMARY KEY (instrument_id, effect_position, param_index)
            );

            CREATE TABLE IF NOT EXISTS vst_plugins (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                plugin_path TEXT NOT NULL,
                kind TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS vst_plugin_params (
                plugin_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                param_index INTEGER NOT NULL,
                name TEXT NOT NULL,
                default_val REAL NOT NULL,
                PRIMARY KEY (plugin_id, position),
                FOREIGN KEY (plugin_id) REFERENCES vst_plugins(id)
            );

            CREATE TABLE IF NOT EXISTS drum_pads (
                instrument_id INTEGER NOT NULL,
                pad_index INTEGER NOT NULL,
                buffer_id INTEGER,
                path TEXT,
                name TEXT NOT NULL DEFAULT '',
                level REAL NOT NULL DEFAULT 0.8,
                reverse INTEGER NOT NULL DEFAULT 0,
                pitch INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (instrument_id, pad_index)
            );

            CREATE TABLE IF NOT EXISTS drum_patterns (
                instrument_id INTEGER NOT NULL,
                pattern_index INTEGER NOT NULL,
                length INTEGER NOT NULL DEFAULT 16,
                swing_amount REAL NOT NULL DEFAULT 0.0,
                chain_enabled INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (instrument_id, pattern_index)
            );

            CREATE TABLE IF NOT EXISTS drum_steps (
                instrument_id INTEGER NOT NULL,
                pattern_index INTEGER NOT NULL,
                pad_index INTEGER NOT NULL,
                step_index INTEGER NOT NULL,
                velocity INTEGER NOT NULL DEFAULT 100,
                probability REAL NOT NULL DEFAULT 1.0,
                pitch_offset INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (instrument_id, pattern_index, pad_index, step_index)
            );

            CREATE TABLE IF NOT EXISTS drum_sequencer_chain (
                instrument_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                pattern_index INTEGER NOT NULL,
                PRIMARY KEY (instrument_id, position)
            );

            CREATE TABLE IF NOT EXISTS chopper_states (
                instrument_id INTEGER PRIMARY KEY,
                buffer_id INTEGER,
                path TEXT,
                name TEXT NOT NULL,
                selected_slice INTEGER NOT NULL,
                next_slice_id INTEGER NOT NULL,
                duration_secs REAL NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chopper_slices (
                instrument_id INTEGER NOT NULL,
                slice_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                start_pos REAL NOT NULL,
                end_pos REAL NOT NULL,
                name TEXT NOT NULL,
                root_note INTEGER NOT NULL,
                PRIMARY KEY (instrument_id, slice_id)
            );

            CREATE TABLE IF NOT EXISTS arrangement_clips (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                instrument_id INTEGER NOT NULL,
                length_ticks INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS arrangement_clip_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                clip_id INTEGER NOT NULL,
                tick INTEGER NOT NULL,
                duration INTEGER NOT NULL,
                pitch INTEGER NOT NULL,
                velocity INTEGER NOT NULL,
                probability REAL NOT NULL DEFAULT 1.0,
                FOREIGN KEY (clip_id) REFERENCES arrangement_clips(id)
            );

            CREATE TABLE IF NOT EXISTS arrangement_placements (
                id INTEGER PRIMARY KEY,
                clip_id INTEGER NOT NULL,
                instrument_id INTEGER NOT NULL,
                start_tick INTEGER NOT NULL,
                length_override INTEGER,
                FOREIGN KEY (clip_id) REFERENCES arrangement_clips(id)
            );

            CREATE TABLE IF NOT EXISTS arrangement_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                play_mode TEXT NOT NULL DEFAULT 'pattern',
                view_start_tick INTEGER NOT NULL DEFAULT 0,
                ticks_per_col INTEGER NOT NULL DEFAULT 120,
                cursor_tick INTEGER NOT NULL DEFAULT 0,
                selected_lane INTEGER NOT NULL DEFAULT 0,
                selected_placement INTEGER
            );

            CREATE TABLE IF NOT EXISTS midi_recording_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                live_input_instrument INTEGER,
                note_passthrough INTEGER NOT NULL,
                channel_filter INTEGER
            );

            CREATE TABLE IF NOT EXISTS midi_cc_mappings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cc_number INTEGER NOT NULL,
                channel INTEGER,
                target_type TEXT NOT NULL,
                target_instrument_id INTEGER NOT NULL,
                target_effect_idx INTEGER,
                target_param_idx INTEGER,
                min_value REAL NOT NULL,
                max_value REAL NOT NULL
            );

            CREATE TABLE IF NOT EXISTS midi_pitch_bend_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                target_type TEXT NOT NULL,
                target_instrument_id INTEGER NOT NULL,
                target_effect_idx INTEGER,
                target_param_idx INTEGER,
                center_value REAL NOT NULL,
                range REAL NOT NULL,
                sensitivity REAL NOT NULL
            );

            -- Clear existing data
            DELETE FROM arrangement_clip_notes;
            DELETE FROM arrangement_placements;
            DELETE FROM arrangement_settings;
            DELETE FROM arrangement_clips;
            DELETE FROM midi_pitch_bend_configs;
            DELETE FROM midi_cc_mappings;
            DELETE FROM midi_recording_settings;
            DELETE FROM chopper_slices;
            DELETE FROM chopper_states;
            DELETE FROM drum_steps;
            DELETE FROM drum_sequencer_chain;
            DELETE FROM drum_patterns;
            DELETE FROM drum_pads;
            DELETE FROM effect_vst_params;
            DELETE FROM instrument_vst_params;
            DELETE FROM vst_plugin_params;
            DELETE FROM vst_plugins;
            DELETE FROM custom_synthdef_params;
            DELETE FROM custom_synthdefs;
            DELETE FROM automation_points;
            DELETE FROM automation_lanes;
            DELETE FROM sampler_slices;
            DELETE FROM sampler_configs;
            DELETE FROM piano_roll_notes;
            DELETE FROM piano_roll_tracks;
            DELETE FROM musical_settings;
            DELETE FROM instrument_modulations;
            DELETE FROM instrument_sends;
            DELETE FROM instrument_effect_params;
            DELETE FROM instrument_effects;
            DELETE FROM instrument_eq_bands;
            DELETE FROM instrument_source_params;
            DELETE FROM instruments;
            DELETE FROM mixer_buses;
            DELETE FROM mixer_master;
            DELETE FROM session;
            ",
    )?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version, applied_at) VALUES (8, datetime('now'))",
        [],
    )?;

    Ok(())
}
