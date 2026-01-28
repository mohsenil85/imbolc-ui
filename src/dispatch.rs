use std::path::PathBuf;

use crate::audio::{self, AudioEngine};
use crate::panes::{MixerPane, PianoRollPane, ServerPane, StripEditPane, StripPane};
use crate::state::{MixerSelection, StripState};
use crate::ui::{Action, Frame, PaneManager};

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
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    app_frame: &mut Frame,
    active_notes: &mut Vec<(u32, u8, u32)>,
) -> bool {
    match action {
        Action::Quit => return true,
        Action::AddStrip(_) => {
            panes.dispatch_to("strip", action);
            if audio_engine.is_running() {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
            panes.switch_to("strip");
        }
        Action::DeleteStrip(strip_id) => {
            let strip_id = *strip_id;
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().remove_strip(strip_id);
            }
            if audio_engine.is_running() {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
        }
        Action::EditStrip(id) => {
            let strip_data = panes
                .get_pane_mut::<StripPane>("strip")
                .and_then(|sp| sp.state().strip(*id).cloned());
            if let Some(strip) = strip_data {
                if let Some(edit) = panes.get_pane_mut::<StripEditPane>("strip_edit") {
                    edit.set_strip(&strip);
                }
                panes.switch_to("strip_edit");
            }
        }
        Action::UpdateStrip(id) => {
            let id = *id;
            // Apply edits from strip_edit pane back to the strip
            let edits = panes.get_pane_mut::<StripEditPane>("strip_edit")
                .map(|edit| {
                    let mut dummy = crate::state::strip::Strip::new(id, crate::state::OscType::Saw);
                    edit.apply_to(&mut dummy);
                    dummy
                });
            if let Some(edited) = edits {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    if let Some(strip) = strip_pane.state_mut().strip_mut(id) {
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
                    let strips: Vec<(u32, bool)> = strip_pane.state().strips.iter()
                        .map(|s| (s.id, s.has_track))
                        .collect();
                    let pr = &mut strip_pane.state_mut().piano_roll;
                    for (sid, has_track) in strips {
                        if has_track && !pr.tracks.contains_key(&sid) {
                            pr.add_track(sid);
                        } else if !has_track && pr.tracks.contains_key(&sid) {
                            pr.remove_track(sid);
                        }
                    }
                }
            }
            if audio_engine.is_running() {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
            // Don't switch pane - stay in edit
        }
        Action::SaveRack => {
            let path = default_rack_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Sync session state
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                app_frame.session.time_signature = strip_pane.state().piano_roll.time_signature;
            }
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                if let Err(e) = strip_pane.state().save(&path, &app_frame.session) {
                    eprintln!("Failed to save: {}", e);
                }
            }
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("default")
                .to_string();
            app_frame.set_project_name(name);
        }
        Action::LoadRack => {
            let path = default_rack_path();
            if path.exists() {
                match StripState::load(&path) {
                    Ok((state, loaded_session)) => {
                        if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                            strip_pane.set_state(state);
                        }
                        app_frame.session = loaded_session;
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
        Action::ConnectServer => {
            let result = audio_engine.connect("127.0.0.1:57110");
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => {
                        let synthdef_dir = std::path::Path::new("synthdefs");
                        if let Err(e) = audio_engine.load_synthdefs(synthdef_dir) {
                            server.set_status(
                                audio::ServerStatus::Connected,
                                &format!("Connected (synthdef warning: {})", e),
                            );
                        } else {
                            server.set_status(audio::ServerStatus::Connected, "Connected");
                        }
                        if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                            let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                        }
                    }
                    Err(e) => {
                        server.set_status(audio::ServerStatus::Error, &e.to_string())
                    }
                }
            }
        }
        Action::DisconnectServer => {
            audio_engine.disconnect();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio_engine.status(), "Disconnected");
                server.set_server_running(audio_engine.server_running());
            }
        }
        Action::StartServer => {
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
        Action::StopServer => {
            audio_engine.stop_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Stopped, "Server stopped");
                server.set_server_running(false);
            }
        }
        Action::CompileSynthDefs => {
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
        Action::LoadSynthDefs => {
            let synthdef_dir = std::path::Path::new("synthdefs");
            let result = audio_engine.load_synthdefs(synthdef_dir);
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => server.set_status(audio_engine.status(), "Synthdefs loaded"),
                    Err(e) => server.set_status(audio_engine.status(), &e),
                }
            }
        }
        Action::SetStripParam(strip_id, ref param, value) => {
            // Real-time param update - update state and audio
            let _ = strip_id;
            let _ = param;
            let _ = value;
            // TODO: implement real-time param setting on audio engine
        }
        Action::MixerMove(delta) => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().mixer_move(*delta);
            }
        }
        Action::MixerJump(direction) => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().mixer_jump(*direction);
            }
        }
        Action::MixerAdjustLevel(delta) => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let state = strip_pane.state_mut();
                match state.mixer_selection {
                    MixerSelection::Strip(idx) => {
                        if let Some(strip) = state.strips.get_mut(idx) {
                            strip.level = (strip.level + delta).clamp(0.0, 1.0);
                        }
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = state.bus_mut(id) {
                            bus.level = (bus.level + delta).clamp(0.0, 1.0);
                        }
                        if let Some(bus) = state.bus(id) {
                            let mute = state.effective_bus_mute(bus);
                            bus_update = Some((id, bus.level, mute, bus.pan));
                        }
                    }
                    MixerSelection::Master => {
                        state.master_level = (state.master_level + delta).clamp(0.0, 1.0);
                    }
                }
            }
            if audio_engine.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                // Rebuild to update strip output levels
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
        }
        Action::MixerToggleMute => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let state = strip_pane.state_mut();
                match state.mixer_selection {
                    MixerSelection::Strip(idx) => {
                        if let Some(strip) = state.strips.get_mut(idx) {
                            strip.mute = !strip.mute;
                        }
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = state.bus_mut(id) {
                            bus.mute = !bus.mute;
                        }
                        if let Some(bus) = state.bus(id) {
                            let mute = state.effective_bus_mute(bus);
                            bus_update = Some((id, bus.level, mute, bus.pan));
                        }
                    }
                    MixerSelection::Master => {
                        state.master_mute = !state.master_mute;
                    }
                }
            }
            if audio_engine.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
        }
        Action::MixerToggleSolo => {
            let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let state = strip_pane.state_mut();
                match state.mixer_selection {
                    MixerSelection::Strip(idx) => {
                        if let Some(strip) = state.strips.get_mut(idx) {
                            strip.solo = !strip.solo;
                        }
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = state.bus_mut(id) {
                            bus.solo = !bus.solo;
                        }
                    }
                    MixerSelection::Master => {}
                }
                for bus in &state.buses {
                    let mute = state.effective_bus_mute(bus);
                    bus_updates.push((bus.id, bus.level, mute, bus.pan));
                }
            }
            if audio_engine.is_running() {
                for (bus_id, level, mute, pan) in bus_updates {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
        }
        Action::MixerCycleSection => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().mixer_cycle_section();
            }
        }
        Action::MixerCycleOutput => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().mixer_cycle_output();
            }
        }
        Action::MixerCycleOutputReverse => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().mixer_cycle_output_reverse();
            }
        }
        Action::MixerAdjustSend(bus_id, delta) => {
            let bus_id = *bus_id;
            let delta = *delta;
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let state = strip_pane.state_mut();
                if let MixerSelection::Strip(idx) = state.mixer_selection {
                    if let Some(strip) = state.strips.get_mut(idx) {
                        if let Some(send) = strip.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                            send.level = (send.level + delta).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
        Action::MixerToggleSend(bus_id) => {
            let bus_id = *bus_id;
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let state = strip_pane.state_mut();
                if let MixerSelection::Strip(idx) = state.mixer_selection {
                    if let Some(strip) = state.strips.get_mut(idx) {
                        if let Some(send) = strip.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                            send.enabled = !send.enabled;
                            if send.enabled && send.level <= 0.0 {
                                send.level = 0.5;
                            }
                        }
                    }
                }
            }
            if audio_engine.is_running() {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                }
            }
        }
        Action::PianoRollToggleNote => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let pitch = pr_pane.cursor_pitch();
                let tick = pr_pane.cursor_tick();
                let dur = pr_pane.default_duration();
                let vel = pr_pane.default_velocity();
                let track = pr_pane.current_track();
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    strip_pane.state_mut().piano_roll.toggle_note(track, pitch, tick, dur, vel);
                }
            }
        }
        Action::PianoRollAdjustDuration(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_duration(delta);
            }
        }
        Action::PianoRollAdjustVelocity(delta) => {
            let delta = *delta;
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.adjust_default_velocity(delta);
            }
        }
        Action::PianoRollPlayStop => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let pr = &mut strip_pane.state_mut().piano_roll;
                pr.playing = !pr.playing;
                if !pr.playing {
                    pr.playhead = 0;
                    if audio_engine.is_running() {
                        audio_engine.release_all_voices();
                    }
                    active_notes.clear();
                }
            }
        }
        Action::PianoRollToggleLoop => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let pr = &mut strip_pane.state_mut().piano_roll;
                pr.looping = !pr.looping;
            }
        }
        Action::PianoRollSetLoopStart => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    strip_pane.state_mut().piano_roll.loop_start = tick;
                }
            }
        }
        Action::PianoRollSetLoopEnd => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    strip_pane.state_mut().piano_roll.loop_end = tick;
                }
            }
        }
        Action::PianoRollChangeTrack(delta) => {
            let delta = *delta;
            let track_count = panes
                .get_pane_mut::<StripPane>("strip")
                .map(|sp| sp.state().piano_roll.track_order.len())
                .unwrap_or(0);
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.change_track(delta, track_count);
            }
        }
        Action::PianoRollCycleTimeSig => {
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                let pr = &mut strip_pane.state_mut().piano_roll;
                pr.time_signature = match pr.time_signature {
                    (4, 4) => (3, 4),
                    (3, 4) => (6, 8),
                    (6, 8) => (5, 4),
                    (5, 4) => (7, 8),
                    _ => (4, 4),
                };
            }
        }
        Action::PianoRollTogglePolyMode => {
            let track_idx = panes
                .get_pane_mut::<PianoRollPane>("piano_roll")
                .map(|pr| pr.current_track());
            if let Some(idx) = track_idx {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    if let Some(track) = strip_pane.state_mut().piano_roll.track_at_mut(idx) {
                        track.polyphonic = !track.polyphonic;
                    }
                }
            }
        }
        Action::PianoRollJump(_direction) => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.jump_to_end();
            }
        }
        Action::PianoRollPlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            // Get the current track's strip_id and polyphonic mode
            let track_info: Option<(u32, bool)> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    panes
                        .get_pane_mut::<StripPane>("strip")
                        .and_then(|sp| sp.state().piano_roll.track_at(idx))
                        .map(|t| (t.module_id, t.polyphonic))
                } else {
                    None
                }
            };

            if let Some((strip_id, polyphonic)) = track_info {
                if audio_engine.is_running() {
                    // Spawn voice
                    let vel_f = velocity as f32 / 127.0;
                    let state = panes
                        .get_pane_mut::<StripPane>("strip")
                        .map(|sp| sp.state().clone());
                    if let Some(state) = state {
                        let _ = audio_engine.spawn_voice(strip_id, pitch, vel_f, 0.0, polyphonic, &state);
                        // Track the note with a fixed duration (one beat = 480 ticks at 120 BPM ~ 0.5s)
                        // We'll use a fixed duration based on current BPM
                        let duration_ticks = 240; // Half beat for staccato feel
                        active_notes.push((strip_id, pitch, duration_ticks));
                    }
                }
            }
        }
        Action::StripPlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            // Get the selected strip's id and polyphonic mode
            let strip_info: Option<(u32, bool)> = panes
                .get_pane_mut::<StripPane>("strip")
                .and_then(|sp| sp.state().selected_strip().map(|s| (s.id, s.polyphonic)));

            if let Some((strip_id, polyphonic)) = strip_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let state = panes
                        .get_pane_mut::<StripPane>("strip")
                        .map(|sp| sp.state().clone());
                    if let Some(state) = state {
                        let _ = audio_engine.spawn_voice(strip_id, pitch, vel_f, 0.0, polyphonic, &state);
                        let duration_ticks = 240;
                        active_notes.push((strip_id, pitch, duration_ticks));
                    }
                }
            }
        }
        Action::UpdateSession(ref session) => {
            app_frame.session = session.clone();
            if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                strip_pane.state_mut().piano_roll.time_signature = session.time_signature;
                strip_pane.state_mut().piano_roll.bpm = session.bpm as f32;
            }
            panes.switch_to("strip");
        }
        _ => {}
    }
    false
}
