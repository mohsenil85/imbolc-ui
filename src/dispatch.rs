use std::path::PathBuf;

use crate::audio::{self, AudioEngine};
use crate::panes::{FileBrowserPane, PianoRollPane, ServerPane, StripEditPane};
use crate::scd_parser;
use crate::state::drum_sequencer::DrumPattern;
use crate::state::{AppState, CustomSynthDef, MixerSelection, ParamSpec};
use crate::ui::{Action, Frame, MixerAction, PaneManager, PianoRollAction, SequencerAction, ServerAction, SessionAction, StripAction};

/// Default path for save file
pub fn default_rack_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("tuidaw")
            .join("default.sqlite")
    } else {
        PathBuf::from("default.sqlite")
    }
}

/// Dispatch an action. Returns true if the app should quit.
pub fn dispatch_action(
    action: &Action,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    app_frame: &mut Frame,
    active_notes: &mut Vec<(u32, u8, u32)>,
) -> bool {
    match action {
        Action::Quit => return true,
        Action::Nav(_) => {} // Handled by PaneManager
        Action::Strip(a) => dispatch_strip(a, state, panes, audio_engine, active_notes),
        Action::Mixer(a) => dispatch_mixer(a, state, audio_engine),
        Action::PianoRoll(a) => dispatch_piano_roll(a, state, panes, audio_engine, active_notes),
        Action::Server(a) => dispatch_server(a, state, panes, audio_engine),
        Action::Session(a) => dispatch_session(a, state, panes, audio_engine, app_frame),
        Action::Sequencer(a) => dispatch_sequencer(a, state, panes, audio_engine),
        Action::None => {}
    }
    false
}

fn dispatch_strip(
    action: &StripAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
) {
    match action {
        StripAction::Add(osc_type) => {
            state.add_strip(*osc_type);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_strip_routing(&state.strip, &state.session);
            }
            panes.switch_to("strip", &*state);
        }
        StripAction::Delete(strip_id) => {
            let strip_id = *strip_id;
            state.remove_strip(strip_id);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_strip_routing(&state.strip, &state.session);
            }
        }
        StripAction::Edit(id) => {
            let strip_data = state.strip.strip(*id).cloned();
            if let Some(strip) = strip_data {
                if let Some(edit) = panes.get_pane_mut::<StripEditPane>("strip_edit") {
                    edit.set_strip(&strip);
                }
                panes.switch_to("strip_edit", &*state);
            }
        }
        StripAction::Update(id) => {
            let id = *id;
            // Apply edits from strip_edit pane back to the strip
            let edits = panes.get_pane_mut::<StripEditPane>("strip_edit")
                .map(|edit| {
                    let mut dummy = crate::state::strip::Strip::new(id, crate::state::OscType::Saw);
                    edit.apply_to(&mut dummy);
                    dummy
                });
            if let Some(edited) = edits {
                if let Some(strip) = state.strip.strip_mut(id) {
                    strip.source = edited.source;
                    strip.source_params = edited.source_params;
                    strip.filter = edited.filter;
                    strip.effects = edited.effects;
                    strip.amp_envelope = edited.amp_envelope;
                    strip.polyphonic = edited.polyphonic;

                    // Handle track toggle
                    if edited.has_track != strip.has_track {
                        strip.has_track = edited.has_track;
                    }
                }
                // Sync piano roll tracks
                let strips: Vec<(u32, bool)> = state.strip.strips.iter()
                    .map(|s| (s.id, s.has_track))
                    .collect();
                let pr = &mut state.session.piano_roll;
                for (sid, has_track) in strips {
                    if has_track && !pr.tracks.contains_key(&sid) {
                        pr.add_track(sid);
                    } else if !has_track && pr.tracks.contains_key(&sid) {
                        pr.remove_track(sid);
                    }
                }
            }
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_strip_routing(&state.strip, &state.session);
            }
            // Don't switch pane - stay in edit
        }
        StripAction::SetParam(strip_id, ref param, value) => {
            // Update state
            if let Some(strip) = state.strip.strip_mut(*strip_id) {
                if let Some(p) = strip.source_params.iter_mut().find(|p| p.name == *param) {
                    p.value = crate::state::ParamValue::Float(*value);
                }
            }
            // Update audio engine in real-time
            if audio_engine.is_running() {
                let _ = audio_engine.set_source_param(*strip_id, param, *value);
            }
        }
        StripAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            // Get the selected strip's id
            let strip_info: Option<u32> = state.strip.selected_strip().map(|s| s.id);

            if let Some(strip_id) = strip_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(strip_id, pitch, vel_f, 0.0, &state.strip, &state.session);
                    let duration_ticks = 240;
                    active_notes.push((strip_id, pitch, duration_ticks));
                }
            }
        }
        StripAction::SelectNext => {
            state.strip.select_next();
        }
        StripAction::SelectPrev => {
            state.strip.select_prev();
        }
        StripAction::SelectFirst => {
            if !state.strip.strips.is_empty() {
                state.strip.selected = Some(0);
            }
        }
        StripAction::SelectLast => {
            if !state.strip.strips.is_empty() {
                state.strip.selected = Some(state.strip.strips.len() - 1);
            }
        }
        StripAction::AddEffect(_, _)
        | StripAction::RemoveEffect(_, _)
        | StripAction::MoveEffect(_, _, _)
        | StripAction::SetFilter(_, _)
        | StripAction::ToggleTrack(_) => {
            // Reserved for future direct dispatch (currently handled inside StripEditPane)
        }
    }
}

fn dispatch_mixer(
    action: &MixerAction,
    state: &mut AppState,
    audio_engine: &mut AudioEngine,
) {
    match action {
        MixerAction::Move(delta) => {
            state.mixer_move(*delta);
        }
        MixerAction::Jump(direction) => {
            state.mixer_jump(*direction);
        }
        MixerAction::AdjustLevel(delta) => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Strip(idx) => {
                    if let Some(strip) = state.strip.strips.get_mut(idx) {
                        strip.level = (strip.level + delta).clamp(0.0, 1.0);
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.level = (bus.level + delta).clamp(0.0, 1.0);
                    }
                    if let Some(bus) = state.session.bus(id) {
                        let mute = state.session.effective_bus_mute(bus);
                        bus_update = Some((id, bus.level, mute, bus.pan));
                    }
                }
                MixerSelection::Master => {
                    state.session.master_level = (state.session.master_level + delta).clamp(0.0, 1.0);
                }
            }
            if audio_engine.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                let _ = audio_engine.update_all_strip_mixer_params(&state.strip, &state.session);
            }
        }
        MixerAction::ToggleMute => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Strip(idx) => {
                    if let Some(strip) = state.strip.strips.get_mut(idx) {
                        strip.mute = !strip.mute;
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.mute = !bus.mute;
                    }
                    if let Some(bus) = state.session.bus(id) {
                        let mute = state.session.effective_bus_mute(bus);
                        bus_update = Some((id, bus.level, mute, bus.pan));
                    }
                }
                MixerSelection::Master => {
                    state.session.master_mute = !state.session.master_mute;
                }
            }
            if audio_engine.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                let _ = audio_engine.update_all_strip_mixer_params(&state.strip, &state.session);
            }
        }
        MixerAction::ToggleSolo => {
            let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
            match state.session.mixer_selection {
                MixerSelection::Strip(idx) => {
                    if let Some(strip) = state.strip.strips.get_mut(idx) {
                        strip.solo = !strip.solo;
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.solo = !bus.solo;
                    }
                }
                MixerSelection::Master => {}
            }
            for bus in &state.session.buses {
                let mute = state.session.effective_bus_mute(bus);
                bus_updates.push((bus.id, bus.level, mute, bus.pan));
            }
            if audio_engine.is_running() {
                for (bus_id, level, mute, pan) in bus_updates {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                let _ = audio_engine.update_all_strip_mixer_params(&state.strip, &state.session);
            }
        }
        MixerAction::CycleSection => {
            state.session.mixer_cycle_section();
        }
        MixerAction::CycleOutput => {
            state.mixer_cycle_output();
        }
        MixerAction::CycleOutputReverse => {
            state.mixer_cycle_output_reverse();
        }
        MixerAction::AdjustSend(bus_id, delta) => {
            let bus_id = *bus_id;
            let delta = *delta;
            if let MixerSelection::Strip(idx) = state.session.mixer_selection {
                if let Some(strip) = state.strip.strips.get_mut(idx) {
                    if let Some(send) = strip.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                        send.level = (send.level + delta).clamp(0.0, 1.0);
                    }
                }
            }
        }
        MixerAction::ToggleSend(bus_id) => {
            let bus_id = *bus_id;
            if let MixerSelection::Strip(idx) = state.session.mixer_selection {
                if let Some(strip) = state.strip.strips.get_mut(idx) {
                    if let Some(send) = strip.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                        send.enabled = !send.enabled;
                        if send.enabled && send.level <= 0.0 {
                            send.level = 0.5;
                        }
                    }
                }
            }
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_strip_routing(&state.strip, &state.session);
            }
        }
    }
}

fn dispatch_piano_roll(
    action: &PianoRollAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
) {
    match action {
        PianoRollAction::ToggleNote => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let pitch = pr_pane.cursor_pitch();
                let tick = pr_pane.cursor_tick();
                let dur = pr_pane.default_duration();
                let vel = pr_pane.default_velocity();
                let track = pr_pane.current_track();
                state.session.piano_roll.toggle_note(track, pitch, tick, dur, vel);
            }
        }
        PianoRollAction::AdjustDuration(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_duration(delta);
            }
        }
        PianoRollAction::AdjustVelocity(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_velocity(delta);
            }
        }
        PianoRollAction::PlayStop => {
            let pr = &mut state.session.piano_roll;
            pr.playing = !pr.playing;
            if !pr.playing {
                pr.playhead = 0;
                if audio_engine.is_running() {
                    audio_engine.release_all_voices();
                }
                active_notes.clear();
            }
            // Clear recording if stopping via normal play/stop
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.set_recording(false);
            }
        }
        PianoRollAction::PlayStopRecord => {
            let is_playing = state.session.piano_roll.playing;

            if !is_playing {
                // Start playing + recording
                state.session.piano_roll.playing = true;
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.set_recording(true);
                }
            } else {
                // Stop playing + recording
                let pr = &mut state.session.piano_roll;
                pr.playing = false;
                pr.playhead = 0;
                if audio_engine.is_running() {
                    audio_engine.release_all_voices();
                }
                active_notes.clear();
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.set_recording(false);
                }
            }
        }
        PianoRollAction::ToggleLoop => {
            state.session.piano_roll.looping = !state.session.piano_roll.looping;
        }
        PianoRollAction::SetLoopStart => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                state.session.piano_roll.loop_start = tick;
            }
        }
        PianoRollAction::SetLoopEnd => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                state.session.piano_roll.loop_end = tick;
            }
        }
        PianoRollAction::ChangeTrack(delta) => {
            let delta = *delta;
            let track_count = state.session.piano_roll.track_order.len();
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.change_track(delta, track_count);
            }
        }
        PianoRollAction::CycleTimeSig => {
            let pr = &mut state.session.piano_roll;
            pr.time_signature = match pr.time_signature {
                (4, 4) => (3, 4),
                (3, 4) => (6, 8),
                (6, 8) => (5, 4),
                (5, 4) => (7, 8),
                _ => (4, 4),
            };
        }
        PianoRollAction::TogglePolyMode => {
            let track_idx = panes
                .get_pane_mut::<PianoRollPane>("piano_roll")
                .map(|pr| pr.current_track());
            if let Some(idx) = track_idx {
                if let Some(track) = state.session.piano_roll.track_at_mut(idx) {
                    track.polyphonic = !track.polyphonic;
                }
            }
        }
        PianoRollAction::Jump(_direction) => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.jump_to_end();
            }
        }
        PianoRollAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            // Get the current track's strip_id
            let track_strip_id: Option<u32> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    state.session.piano_roll.track_at(idx).map(|t| t.module_id)
                } else {
                    None
                }
            };

            if let Some(strip_id) = track_strip_id {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(strip_id, pitch, vel_f, 0.0, &state.strip, &state.session);
                    let duration_ticks = 240; // Half beat for staccato feel
                    active_notes.push((strip_id, pitch, duration_ticks));
                }

                // Record note if recording
                let recording_info = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .filter(|pr| pr.is_recording())
                    .map(|pr| (pr.current_track(), pr.default_duration(), pr.default_velocity()));
                if let Some((track_idx, duration, vel)) = recording_info {
                    let playhead = state.session.piano_roll.playhead;
                    state.session.piano_roll.toggle_note(track_idx, pitch, playhead, duration, vel);
                }
            }
        }
        PianoRollAction::MoveCursor(_, _)
        | PianoRollAction::SetBpm(_)
        | PianoRollAction::Zoom(_)
        | PianoRollAction::ScrollOctave(_) => {
            // Reserved for future direct dispatch (currently handled inside PianoRollPane)
        }
    }
}

fn dispatch_server(
    action: &ServerAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        ServerAction::Connect => {
            let result = audio_engine.connect("127.0.0.1:57110");
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => {
                        // Load built-in synthdefs
                        let synthdef_dir = std::path::Path::new("synthdefs");
                        let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

                        // Also load custom synthdefs from config dir
                        let config_dir = config_synthdefs_dir();
                        let custom_result = if config_dir.exists() {
                            audio_engine.load_synthdefs(&config_dir)
                        } else {
                            Ok(())
                        };

                        // Load drum sequencer samples
                        for pad in &state.session.drum_sequencer.pads {
                            if let Some(buffer_id) = pad.buffer_id {
                                if let Some(ref path) = pad.path {
                                    let _ = audio_engine.load_sample(buffer_id, path);
                                }
                            }
                        }

                        match (builtin_result, custom_result) {
                            (Ok(()), Ok(())) => {
                                server.set_status(audio::ServerStatus::Connected, "Connected");
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                server.set_status(
                                    audio::ServerStatus::Connected,
                                    &format!("Connected (synthdef warning: {})", e),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        server.set_status(audio::ServerStatus::Error, &e.to_string())
                    }
                }
            }
        }
        ServerAction::Disconnect => {
            audio_engine.disconnect();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio_engine.status(), "Disconnected");
                server.set_server_running(audio_engine.server_running());
            }
        }
        ServerAction::Start => {
            let result = audio_engine.start_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => {
                        server.set_status(audio::ServerStatus::Running, "Server started");
                        server.set_server_running(true);
                    }
                    Err(e) => {
                        server.set_status(audio::ServerStatus::Error, &e);
                        server.set_server_running(false);
                    }
                }
            }
        }
        ServerAction::Stop => {
            audio_engine.stop_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Stopped, "Server stopped");
                server.set_server_running(false);
            }
        }
        ServerAction::CompileSynthDefs => {
            let scd_path = std::path::Path::new("synthdefs/compile.scd");
            match audio_engine.compile_synthdefs_async(scd_path) {
                Ok(()) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio_engine.status(), "Compiling synthdefs...");
                    }
                }
                Err(e) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio_engine.status(), &e);
                    }
                }
            }
        }
        ServerAction::LoadSynthDefs => {
            // Load built-in synthdefs
            let synthdef_dir = std::path::Path::new("synthdefs");
            let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

            // Also load custom synthdefs from config dir
            let config_dir = config_synthdefs_dir();
            let custom_result = if config_dir.exists() {
                audio_engine.load_synthdefs(&config_dir)
            } else {
                Ok(())
            };

            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match (builtin_result, custom_result) {
                    (Ok(()), Ok(())) => {
                        server.set_status(audio_engine.status(), "Synthdefs loaded (built-in + custom)");
                    }
                    (Err(e), _) => {
                        server.set_status(audio_engine.status(), &format!("Error loading built-in: {}", e));
                    }
                    (_, Err(e)) => {
                        server.set_status(audio_engine.status(), &format!("Error loading custom: {}", e));
                    }
                }
            }
        }
    }
}

fn dispatch_session(
    action: &SessionAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    app_frame: &mut Frame,
) {
    match action {
        SessionAction::Save => {
            let path = default_rack_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Sync piano roll time_signature from session
            state.session.piano_roll.time_signature = state.session.time_signature;
            if let Err(e) = crate::state::persistence::save_project(&path, &state.session, &state.strip) {
                eprintln!("Failed to save: {}", e);
            }
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("default")
                .to_string();
            app_frame.set_project_name(name);
        }
        SessionAction::Load => {
            let path = default_rack_path();
            if path.exists() {
                match crate::state::persistence::load_project(&path) {
                    Ok((loaded_session, loaded_strip)) => {
                        state.session = loaded_session;
                        state.strip = loaded_strip;
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("default")
                            .to_string();
                        app_frame.set_project_name(name);
                    }
                    Err(e) => {
                        eprintln!("Failed to load: {}", e);
                    }
                }
            }
        }
        SessionAction::UpdateSession(ref settings) => {
            state.session.apply_musical_settings(settings);
            state.session.piano_roll.time_signature = state.session.time_signature;
            state.session.piano_roll.bpm = state.session.bpm as f32;
            panes.switch_to("strip", &*state);
        }
        SessionAction::OpenFileBrowser(ref file_action) => {
            if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                fb.open_for(file_action.clone(), None);
            }
            panes.push_to("file_browser", &*state);
        }
        SessionAction::ImportCustomSynthDef(ref path) => {
            // Read and parse the .scd file
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match scd_parser::parse_scd_file(&content) {
                        Ok(parsed) => {
                            // Create params with inferred ranges
                            let params: Vec<ParamSpec> = parsed
                                .params
                                .iter()
                                .map(|(name, default)| {
                                    let (min, max) =
                                        scd_parser::infer_param_range(name, *default);
                                    ParamSpec {
                                        name: name.clone(),
                                        default: *default,
                                        min,
                                        max,
                                    }
                                })
                                .collect();

                            // Create the custom synthdef entry
                            let synthdef_name = parsed.name.clone();
                            let custom = CustomSynthDef {
                                id: 0, // Will be set by registry.add()
                                name: parsed.name.clone(),
                                synthdef_name: synthdef_name.clone(),
                                source_path: path.clone(),
                                params,
                            };

                            // Register it
                            let _id = state.session.custom_synthdefs.add(custom);

                            // Copy the .scd file to the config synthdefs directory
                            let config_dir = config_synthdefs_dir();
                            let _ = std::fs::create_dir_all(&config_dir);

                            // Copy .scd file
                            if let Some(filename) = path.file_name() {
                                let dest = config_dir.join(filename);
                                let _ = std::fs::copy(path, &dest);
                            }

                            // Compile and load the synthdef
                            match compile_and_load_synthdef(path, &config_dir, &synthdef_name, audio_engine) {
                                Ok(_) => {
                                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                        server.set_status(audio_engine.status(), &format!("Loaded custom synthdef: {}", synthdef_name));
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to compile/load synthdef: {}", e);
                                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                        server.set_status(audio_engine.status(), &format!("Import error: {}", e));
                                    }
                                }
                            }

                            // Pop back to the pane that opened the file browser
                            panes.pop(&*state);
                        }
                        Err(e) => {
                            eprintln!("Failed to parse .scd file: {}", e);
                            panes.pop(&*state);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read .scd file: {}", e);
                    panes.pop(&*state);
                }
            }
        }
    }
}

fn dispatch_sequencer(
    action: &SequencerAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        SequencerAction::ToggleStep(pad_idx, step_idx) => {
            if let Some(step) = state
                .session
                .drum_sequencer
                .pattern_mut()
                .steps
                .get_mut(*pad_idx)
                .and_then(|s| s.get_mut(*step_idx))
            {
                step.active = !step.active;
            }
        }
        SequencerAction::AdjustVelocity(pad_idx, step_idx, delta) => {
            if let Some(step) = state
                .session
                .drum_sequencer
                .pattern_mut()
                .steps
                .get_mut(*pad_idx)
                .and_then(|s| s.get_mut(*step_idx))
            {
                step.velocity = (step.velocity as i16 + *delta as i16).clamp(1, 127) as u8;
            }
        }
        SequencerAction::ClearPad(pad_idx) => {
            for step in state
                .session
                .drum_sequencer
                .pattern_mut()
                .steps
                .get_mut(*pad_idx)
                .iter_mut()
                .flat_map(|s| s.iter_mut())
            {
                step.active = false;
            }
        }
        SequencerAction::ClearPattern => {
            let len = state.session.drum_sequencer.pattern().length;
            *state.session.drum_sequencer.pattern_mut() = DrumPattern::new(len);
        }
        SequencerAction::CyclePatternLength => {
            let lengths = [8, 16, 32, 64];
            let current = state.session.drum_sequencer.pattern().length;
            let idx = lengths.iter().position(|&l| l == current).unwrap_or(0);
            let new_len = lengths[(idx + 1) % lengths.len()];
            let old_pattern = state.session.drum_sequencer.pattern().clone();
            let mut new_pattern = DrumPattern::new(new_len);
            for (pad_idx, old_steps) in old_pattern.steps.iter().enumerate() {
                for (step_idx, step) in old_steps.iter().enumerate() {
                    if step_idx < new_len {
                        new_pattern.steps[pad_idx][step_idx] = step.clone();
                    }
                }
            }
            *state.session.drum_sequencer.pattern_mut() = new_pattern;
        }
        SequencerAction::NextPattern => {
            let seq = &mut state.session.drum_sequencer;
            seq.current_pattern = (seq.current_pattern + 1) % seq.patterns.len();
        }
        SequencerAction::PrevPattern => {
            let seq = &mut state.session.drum_sequencer;
            seq.current_pattern = if seq.current_pattern == 0 {
                seq.patterns.len() - 1
            } else {
                seq.current_pattern - 1
            };
        }
        SequencerAction::AdjustPadLevel(pad_idx, delta) => {
            if let Some(pad) = state.session.drum_sequencer.pads.get_mut(*pad_idx) {
                pad.level = (pad.level + delta).clamp(0.0, 1.0);
            }
        }
        SequencerAction::PlayStop => {
            let seq = &mut state.session.drum_sequencer;
            seq.playing = !seq.playing;
            if !seq.playing {
                seq.current_step = 0;
                seq.step_accumulator = 0.0;
            }
        }
        SequencerAction::LoadSample(pad_idx) => {
            if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                fb.open_for(
                    crate::ui::FileSelectAction::LoadDrumSample(*pad_idx),
                    None,
                );
            }
            panes.push_to("file_browser", &*state);
        }
        SequencerAction::LoadSampleResult(pad_idx, path) => {
            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let seq = &mut state.session.drum_sequencer;
            let buffer_id = seq.next_buffer_id;
            seq.next_buffer_id += 1;

            if audio_engine.is_running() {
                let _ = audio_engine.load_sample(buffer_id, &path_str);
            }

            if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                pad.buffer_id = Some(buffer_id);
                pad.path = Some(path_str);
                pad.name = name;
            }

            panes.pop(&*state);
        }
    }
}

/// Get the config directory for custom synthdefs
fn config_synthdefs_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("tuidaw")
            .join("synthdefs")
    } else {
        PathBuf::from("synthdefs")
    }
}

/// Find sclang executable, checking common locations
fn find_sclang() -> Option<PathBuf> {
    // Check if sclang is in PATH
    if let Ok(output) = std::process::Command::new("which").arg("sclang").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Common macOS locations
    let candidates = [
        "/Applications/SuperCollider.app/Contents/MacOS/sclang",
        "/Applications/SuperCollider/SuperCollider.app/Contents/MacOS/sclang",
        "/usr/local/bin/sclang",
        "/opt/homebrew/bin/sclang",
    ];

    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Compile a .scd file using sclang and load it into scsynth
fn compile_and_load_synthdef(
    scd_path: &std::path::Path,
    output_dir: &std::path::Path,
    synthdef_name: &str,
    audio_engine: &mut AudioEngine,
) -> Result<(), String> {
    // Find sclang
    let sclang = find_sclang().ok_or_else(|| {
        "sclang not found. Install SuperCollider or add sclang to PATH.".to_string()
    })?;

    // Read the original .scd file
    let scd_content = std::fs::read_to_string(scd_path)
        .map_err(|e| format!("Failed to read .scd file: {}", e))?;

    // Replace directory references with the actual output directory
    // Handle both patterns: `dir ? thisProcess...` and just `thisProcess...`
    let output_dir_str = format!("\"{}\"", output_dir.display());
    let modified_content = scd_content
        .replace("dir ? thisProcess.nowExecutingPath.dirname", &output_dir_str)
        .replace("thisProcess.nowExecutingPath.dirname", &output_dir_str);

    // Wrap in a block that exits when done
    let compile_script = format!(
        "(\n{}\n\"SUCCESS\".postln;\n0.exit;\n)",
        modified_content
    );

    // Write temp compile script
    let temp_script = std::env::temp_dir().join("tuidaw_compile_custom.scd");
    std::fs::write(&temp_script, &compile_script)
        .map_err(|e| format!("Failed to write compile script: {}", e))?;

    // Run sclang with a timeout by spawning and waiting
    let mut child = std::process::Command::new(&sclang)
        .arg(&temp_script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run sclang: {}", e))?;

    // Wait up to 30 seconds for compilation
    let timeout = std::time::Duration::from_secs(30);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("sclang compilation timed out".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Error waiting for sclang: {}", e)),
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to get sclang output: {}", e))?;

    // Check for errors (but ignore common non-error messages)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Look for actual errors, not just any "ERROR" in output
    let has_error = stderr.lines().any(|line| {
        line.contains("ERROR:") || line.contains("FAILURE")
    }) || stdout.lines().any(|line| {
        line.starts_with("ERROR:") || line.contains("FAILURE")
    });

    if has_error {
        return Err(format!("sclang error: {}{}", stdout, stderr));
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_script);

    // Load the .scsyndef into scsynth if connected
    if audio_engine.is_running() {
        let scsyndef_path = output_dir.join(format!("{}.scsyndef", synthdef_name));
        if scsyndef_path.exists() {
            audio_engine.load_synthdef_file(&scsyndef_path)?;
        } else {
            // Try loading all synthdefs from the directory as fallback
            audio_engine.load_synthdefs(output_dir)?;
        }
    }

    Ok(())
}
