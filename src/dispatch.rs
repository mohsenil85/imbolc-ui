use std::path::PathBuf;

use crate::audio::{self, AudioEngine};
use crate::panes::{EditPane, PianoRollPane, RackPane, ServerPane};
use crate::state::{MixerSelection, RackState};
use crate::ui::{Action, Frame, PaneManager};

/// Default path for rack save file
pub fn default_rack_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("tuidaw")
            .join("rack.tuidaw")
    } else {
        PathBuf::from("rack.tuidaw")
    }
}

/// Dispatch an action. Returns true if the app should quit.
pub fn dispatch_action(
    action: &Action,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    _app_frame: &mut Frame,
    active_notes: &mut Vec<(u32, u8, u32)>,
) -> bool {
    match action {
        Action::Quit => return true,
        Action::AddModule(_) => {
            panes.dispatch_to("rack", action);
            if audio_engine.is_running() {
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    let _ = audio_engine.rebuild_routing(rack_pane.rack());
                }
            }
            panes.switch_to("rack");
        }
        Action::DeleteModule(module_id) => {
            if audio_engine.is_running() {
                let _ = audio_engine.free_synth(*module_id);
            }
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().remove_module(*module_id);
            }
            if audio_engine.is_running() {
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    let _ = audio_engine.rebuild_routing(rack_pane.rack());
                }
            }
        }
        Action::EditModule(id) => {
            let module_data = panes
                .get_pane_mut::<RackPane>("rack")
                .and_then(|rack| rack.get_module_for_edit(*id));
            if let Some((id, name, type_name, params)) = module_data {
                if let Some(edit) = panes.get_pane_mut::<EditPane>("edit") {
                    edit.set_module(id, name, type_name, params);
                }
                panes.switch_to("edit");
            }
        }
        Action::UpdateModuleParams(_, _) => {
            panes.dispatch_to("rack", action);
            panes.switch_to("rack");
        }
        Action::SaveRack => {
            let path = default_rack_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                if let Err(e) = rack_pane.rack().save(&path) {
                    eprintln!("Failed to save rack: {}", e);
                }
            }
        }
        Action::LoadRack => {
            let path = default_rack_path();
            if path.exists() {
                match RackState::load(&path) {
                    Ok(rack) => {
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            rack_pane.set_rack(rack);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to load rack: {}", e);
                    }
                }
            }
        }
        Action::AddConnection(_) | Action::RemoveConnection(_) => {
            panes.dispatch_to("rack", action);
            if audio_engine.is_running() {
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    let _ = audio_engine.rebuild_routing(rack_pane.rack());
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
        Action::SetModuleParam(module_id, ref param, value) => {
            if audio_engine.is_running() {
                let _ = audio_engine.set_param(*module_id, param, *value);
            }
        }
        Action::MixerMove(delta) => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().mixer.move_selection(*delta);
            }
        }
        Action::MixerJump(direction) => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().mixer.jump_selection(*direction);
            }
        }
        Action::MixerAdjustLevel(delta) => {
            let mut updates: Vec<(u32, f32, bool)> = Vec::new();
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let mixer = &mut rack_pane.rack_mut().mixer;
                match mixer.selection {
                    MixerSelection::Channel(id) => {
                        if let Some(ch) = mixer.channel_mut(id) {
                            ch.level = (ch.level + delta).clamp(0.0, 1.0);
                        }
                        updates = mixer.collect_channel_updates();
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = mixer.bus_mut(id) {
                            bus.level = (bus.level + delta).clamp(0.0, 1.0);
                        }
                        if let Some(bus) = mixer.bus(id) {
                            let mute = mixer.effective_bus_mute(bus);
                            bus_update = Some((id, bus.level, mute, bus.pan));
                        }
                    }
                    MixerSelection::Master => {
                        mixer.master_level = (mixer.master_level + delta).clamp(0.0, 1.0);
                        updates = mixer.collect_channel_updates();
                    }
                }
            }
            if audio_engine.is_running() {
                for (module_id, level, mute) in updates {
                    let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                }
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
        }
        Action::MixerToggleMute => {
            let mut updates: Vec<(u32, f32, bool)> = Vec::new();
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let mixer = &mut rack_pane.rack_mut().mixer;
                match mixer.selection {
                    MixerSelection::Channel(id) => {
                        if let Some(ch) = mixer.channel_mut(id) {
                            ch.mute = !ch.mute;
                        }
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = mixer.bus_mut(id) {
                            bus.mute = !bus.mute;
                        }
                        if let Some(bus) = mixer.bus(id) {
                            let mute = mixer.effective_bus_mute(bus);
                            bus_update = Some((id, bus.level, mute, bus.pan));
                        }
                    }
                    MixerSelection::Master => {
                        mixer.master_mute = !mixer.master_mute;
                    }
                }
                updates = mixer.collect_channel_updates();
            }
            if audio_engine.is_running() {
                for (module_id, level, mute) in updates {
                    let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                }
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
        }
        Action::MixerToggleSolo => {
            let mut updates: Vec<(u32, f32, bool)> = Vec::new();
            let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let mixer = &mut rack_pane.rack_mut().mixer;
                match mixer.selection {
                    MixerSelection::Channel(id) => {
                        if let Some(ch) = mixer.channel_mut(id) {
                            ch.solo = !ch.solo;
                        }
                    }
                    MixerSelection::Bus(id) => {
                        if let Some(bus) = mixer.bus_mut(id) {
                            bus.solo = !bus.solo;
                        }
                    }
                    MixerSelection::Master => {}
                }
                updates = mixer.collect_channel_updates();
                for bus in &mixer.buses {
                    let mute = mixer.effective_bus_mute(bus);
                    bus_updates.push((bus.id, bus.level, mute, bus.pan));
                }
            }
            if audio_engine.is_running() {
                for (module_id, level, mute) in updates {
                    let _ = audio_engine.set_output_mixer_params(module_id, level, mute);
                }
                for (bus_id, level, mute, pan) in bus_updates {
                    let _ = audio_engine.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
        }
        Action::MixerCycleSection => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().mixer.cycle_section();
            }
        }
        Action::MixerCycleOutput => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().mixer.cycle_output();
            }
        }
        Action::MixerCycleOutputReverse => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                rack_pane.rack_mut().mixer.cycle_output_reverse();
            }
        }
        Action::MixerAdjustSend(bus_id, delta) => {
            let bus_id = *bus_id;
            let delta = *delta;
            let mut send_update: Option<(u8, u8, f32)> = None;
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let mixer = &mut rack_pane.rack_mut().mixer;
                if let MixerSelection::Channel(ch_id) = mixer.selection {
                    if let Some(ch) = mixer.channel_mut(ch_id) {
                        if let Some(send) = ch.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                            send.level = (send.level + delta).clamp(0.0, 1.0);
                            send_update = Some((ch_id, bus_id, send.level));
                        }
                    }
                }
            }
            if let Some((ch_id, bus_id, level)) = send_update {
                if audio_engine.is_running() {
                    let _ = audio_engine.set_send_level(ch_id, bus_id, level);
                }
            }
        }
        Action::MixerToggleSend(bus_id) => {
            let bus_id = *bus_id;
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let mixer = &mut rack_pane.rack_mut().mixer;
                if let MixerSelection::Channel(ch_id) = mixer.selection {
                    if let Some(ch) = mixer.channel_mut(ch_id) {
                        if let Some(send) = ch.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                            send.enabled = !send.enabled;
                            if send.enabled && send.level <= 0.0 {
                                send.level = 0.5;
                            }
                        }
                    }
                }
            }
            if audio_engine.is_running() {
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    let _ = audio_engine.rebuild_routing(rack_pane.rack());
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
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    rack_pane.rack_mut().piano_roll.toggle_note(track, pitch, tick, dur, vel);
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
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let pr = &mut rack_pane.rack_mut().piano_roll;
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
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let pr = &mut rack_pane.rack_mut().piano_roll;
                pr.looping = !pr.looping;
            }
        }
        Action::PianoRollSetLoopStart => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    rack_pane.rack_mut().piano_roll.loop_start = tick;
                }
            }
        }
        Action::PianoRollSetLoopEnd => {
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                let tick = pr_pane.cursor_tick();
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    rack_pane.rack_mut().piano_roll.loop_end = tick;
                }
            }
        }
        Action::PianoRollChangeTrack(delta) => {
            let delta = *delta;
            let track_count = panes
                .get_pane_mut::<RackPane>("rack")
                .map(|r| r.rack().piano_roll.track_order.len())
                .unwrap_or(0);
            if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                pr_pane.change_track(delta, track_count);
            }
        }
        Action::PianoRollCycleTimeSig => {
            if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                let pr = &mut rack_pane.rack_mut().piano_roll;
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
                if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                    if let Some(track) = rack_pane.rack_mut().piano_roll.track_at_mut(idx) {
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
        _ => {}
    }
    false
}
