use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audio::{self, AudioEngine};
use crate::panes::{FileBrowserPane, InstrumentEditPane, PianoRollPane, ServerPane};
use crate::scd_parser;
use crate::state::drum_sequencer::{ChopperState, DrumPattern};
use crate::state::sampler::Slice;
use crate::state::{AppState, CustomSynthDef, MixerSelection, ParamSpec};
use crate::ui::{Action, ChopperAction, Frame, InstrumentAction, MixerAction, PaneManager, PianoRollAction, SequencerAction, ServerAction, SessionAction};

/// Default path for save file
pub fn default_rack_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("ilex")
            .join("default.sqlite")
    } else {
        PathBuf::from("default.sqlite")
    }
}

/// Generate a timestamped path for a recording file in the current directory
fn recording_path(prefix: &str) -> PathBuf {
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    dir.join(format!("{}_{}.wav", prefix, secs))
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
        Action::Instrument(a) => dispatch_instrument(a, state, panes, audio_engine, active_notes),
        Action::Mixer(a) => dispatch_mixer(a, state, audio_engine),
        Action::PianoRoll(a) => dispatch_piano_roll(a, state, panes, audio_engine, active_notes),
        Action::Server(a) => dispatch_server(a, state, panes, audio_engine),
        Action::Session(a) => dispatch_session(a, state, panes, audio_engine, app_frame),
        Action::Sequencer(a) => dispatch_sequencer(a, state, panes, audio_engine),
        Action::Chopper(a) => dispatch_chopper(a, state, panes, audio_engine),
        Action::None => {}
        // Layer management actions — handled in main.rs before dispatch
        Action::ExitPerformanceMode | Action::PushLayer(_) | Action::PopLayer(_) => {}
    }
    false
}

fn dispatch_instrument(
    action: &InstrumentAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    active_notes: &mut Vec<(u32, u8, u32)>,
) {
    match action {
        InstrumentAction::Add(osc_type) => {
            state.add_instrument(*osc_type);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
            panes.switch_to("instrument", &*state);
        }
        InstrumentAction::Delete(inst_id) => {
            let inst_id = *inst_id;
            state.remove_instrument(inst_id);
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
        }
        InstrumentAction::Edit(id) => {
            let inst_data = state.instruments.instrument(*id).cloned();
            if let Some(inst) = inst_data {
                if let Some(edit) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
                    edit.set_instrument(&inst);
                }
                panes.switch_to("instrument_edit", &*state);
            }
        }
        InstrumentAction::Update(id) => {
            let id = *id;
            // Apply edits from instrument_edit pane back to the instrument
            let edits = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit")
                .map(|edit| {
                    let mut dummy = crate::state::instrument::Instrument::new(id, crate::state::SourceType::Saw);
                    edit.apply_to(&mut dummy);
                    dummy
                });
            if let Some(edited) = edits {
                if let Some(instrument) = state.instruments.instrument_mut(id) {
                    instrument.source = edited.source;
                    instrument.source_params = edited.source_params;
                    instrument.filter = edited.filter;
                    instrument.effects = edited.effects;
                    instrument.amp_envelope = edited.amp_envelope;
                    instrument.polyphonic = edited.polyphonic;
                    instrument.active = edited.active;
                }
            }
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
            }
            // Don't switch pane - stay in edit
        }
        InstrumentAction::SetParam(instrument_id, ref param, value) => {
            // Update state
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                if let Some(p) = instrument.source_params.iter_mut().find(|p| p.name == *param) {
                    p.value = crate::state::ParamValue::Float(*value);
                }
            }
            // Update audio engine in real-time
            if audio_engine.is_running() {
                let _ = audio_engine.set_source_param(*instrument_id, param, *value);
            }
        }
        InstrumentAction::PlayNote(pitch, velocity) => {
            let pitch = *pitch;
            let velocity = *velocity;
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    let duration_ticks = 240;
                    active_notes.push((instrument_id, pitch, duration_ticks));
                }
            }
        }
        InstrumentAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let instrument_info: Option<u32> = state.instruments.selected_instrument().map(|s| s.id);

            if let Some(instrument_id) = instrument_info {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                        active_notes.push((instrument_id, pitch, 240));
                    }
                }
            }
        }
        InstrumentAction::Select(idx) => {
            if *idx < state.instruments.instruments.len() {
                state.instruments.selected = Some(*idx);
            }
        }
        InstrumentAction::SelectNext => {
            state.instruments.select_next();
        }
        InstrumentAction::SelectPrev => {
            state.instruments.select_prev();
        }
        InstrumentAction::SelectFirst => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(0);
            }
        }
        InstrumentAction::SelectLast => {
            if !state.instruments.instruments.is_empty() {
                state.instruments.selected = Some(state.instruments.instruments.len() - 1);
            }
        }
        InstrumentAction::PlayDrumPad(pad_idx) => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                if let Some(seq) = &instrument.drum_sequencer {
                    if let Some(pad) = seq.pads.get(*pad_idx) {
                        if let (Some(buffer_id), instrument_id) = (pad.buffer_id, instrument.id) {
                            let amp = pad.level;
                            if audio_engine.is_running() {
                                let _ = audio_engine.play_drum_hit_to_instrument(
                                    buffer_id, amp, instrument_id,
                                    pad.slice_start, pad.slice_end,
                                );
                            }
                        }
                    }
                }
            }
        }
        InstrumentAction::LoadSampleResult(instrument_id, ref path) => {
            let instrument_id = *instrument_id;
            let path_str = path.to_string_lossy().to_string();
            let sample_name = path.file_stem()
                .map(|s| s.to_string_lossy().to_string());

            let buffer_id = state.instruments.next_sampler_buffer_id;
            state.instruments.next_sampler_buffer_id += 1;

            if audio_engine.is_running() {
                let _ = audio_engine.load_sample(buffer_id, &path_str);
            }

            if let Some(instrument) = state.instruments.instrument_mut(instrument_id) {
                if let Some(ref mut config) = instrument.sampler_config {
                    config.buffer_id = Some(buffer_id);
                    config.sample_name = sample_name;
                }
            }

            panes.pop(&*state);

            // Refresh the instrument edit pane with updated sample info
            let inst_data = state.instruments.instrument(instrument_id).cloned();
            if let Some(inst) = inst_data {
                if let Some(edit) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
                    let saved_row = edit.selected_row;
                    edit.set_instrument(&inst);
                    edit.selected_row = saved_row;
                }
            }
        }
        InstrumentAction::AddEffect(_, _)
        | InstrumentAction::RemoveEffect(_, _)
        | InstrumentAction::MoveEffect(_, _, _)
        | InstrumentAction::SetFilter(_, _) => {
            // Reserved for future direct dispatch (currently handled inside InstrumentEditPane)
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
        MixerAction::SelectAt(selection) => {
            state.session.mixer_selection = *selection;
        }
        MixerAction::AdjustLevel(delta) => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.level = (instrument.level + delta).clamp(0.0, 1.0);
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
                let _ = audio_engine.update_all_instrument_mixer_params(&state.instruments, &state.session);
            }
        }
        MixerAction::ToggleMute => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.mute = !instrument.mute;
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
                let _ = audio_engine.update_all_instrument_mixer_params(&state.instruments, &state.session);
            }
        }
        MixerAction::ToggleSolo => {
            let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.solo = !instrument.solo;
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
                let _ = audio_engine.update_all_instrument_mixer_params(&state.instruments, &state.session);
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
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                    if let Some(send) = instrument.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                        send.level = (send.level + delta).clamp(0.0, 1.0);
                    }
                }
            }
        }
        MixerAction::ToggleSend(bus_id) => {
            let bus_id = *bus_id;
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                    if let Some(send) = instrument.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                        send.enabled = !send.enabled;
                        if send.enabled && send.level <= 0.0 {
                            send.level = 0.5;
                        }
                    }
                }
            }
            if audio_engine.is_running() {
                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
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
            // Get the current track's instrument_id
            let track_instrument_id: Option<u32> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    state.session.piano_roll.track_at(idx).map(|t| t.module_id)
                } else {
                    None
                }
            };

            if let Some(instrument_id) = track_instrument_id {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                    let duration_ticks = 240; // Half beat for staccato feel
                    active_notes.push((instrument_id, pitch, duration_ticks));
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
        PianoRollAction::PlayNotes(ref pitches, velocity) => {
            let velocity = *velocity;
            let track_instrument_id: Option<u32> = {
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|pr| pr.current_track());
                if let Some(idx) = track_idx {
                    state.session.piano_roll.track_at(idx).map(|t| t.module_id)
                } else {
                    None
                }
            };

            if let Some(instrument_id) = track_instrument_id {
                if audio_engine.is_running() {
                    let vel_f = velocity as f32 / 127.0;
                    for &pitch in pitches {
                        let _ = audio_engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &state.instruments, &state.session);
                        active_notes.push((instrument_id, pitch, 240));
                    }
                }

                // Record chord notes if recording
                let recording_info = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .filter(|pr| pr.is_recording())
                    .map(|pr| (pr.current_track(), pr.default_duration(), pr.default_velocity()));
                if let Some((track_idx, duration, vel)) = recording_info {
                    let playhead = state.session.piano_roll.playhead;
                    for &pitch in pitches {
                        state.session.piano_roll.toggle_note(track_idx, pitch, playhead, duration, vel);
                    }
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

                        // Load drum sequencer samples for all drum machine instruments
                        for instrument in &state.instruments.instruments {
                            if let Some(seq) = &instrument.drum_sequencer {
                                for pad in &seq.pads {
                                    if let Some(buffer_id) = pad.buffer_id {
                                        if let Some(ref path) = pad.path {
                                            let _ = audio_engine.load_sample(buffer_id, path);
                                        }
                                    }
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
            let (input_dev, output_dev) = panes.get_pane_mut::<ServerPane>("server")
                .map(|s| (s.selected_input_device(), s.selected_output_device()))
                .unwrap_or((None, None));
            let result = audio_engine.start_server_with_devices(
                input_dev.as_deref(),
                output_dev.as_deref(),
            );
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
        ServerAction::RecordMaster => {
            if audio_engine.is_recording() {
                if let Some(path) = audio_engine.stop_recording() {
                    // Auto-deactivate AudioIn instrument on stop
                    if let Some(inst) = state.instruments.selected_instrument_mut() {
                        if inst.source.is_audio_input() && inst.active {
                            inst.active = false;
                            let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                        }
                    }
                    // Defer waveform load — scsynth needs time to flush the WAV
                    state.pending_recording_path = Some(path.clone());
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(
                            audio_engine.status(),
                            &format!("Recording saved: {}", path.display()),
                        );
                    }
                }
            } else if audio_engine.is_running() {
                // Auto-activate AudioIn instrument on start
                if let Some(inst) = state.instruments.selected_instrument_mut() {
                    if inst.source.is_audio_input() && !inst.active {
                        inst.active = true;
                        let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                }
                let path = recording_path("master");
                match audio_engine.start_recording(0, &path) {
                    Ok(()) => {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(
                                audio_engine.status(),
                                &format!("Recording to {}", path.display()),
                            );
                        }
                    }
                    Err(e) => {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(audio_engine.status(), &format!("Record error: {}", e));
                        }
                    }
                }
            }
        }
        ServerAction::RecordInput => {
            if audio_engine.is_recording() {
                if let Some(path) = audio_engine.stop_recording() {
                    // Auto-deactivate AudioIn instrument on stop
                    if let Some(inst) = state.instruments.selected_instrument_mut() {
                        if inst.source.is_audio_input() && inst.active {
                            inst.active = false;
                            let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                        }
                    }
                    // Defer waveform load — scsynth needs time to flush the WAV
                    state.pending_recording_path = Some(path.clone());
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(
                            audio_engine.status(),
                            &format!("Recording saved: {}", path.display()),
                        );
                    }
                }
            } else if audio_engine.is_running() {
                // Record from the selected instrument's source_out bus
                if let Some(inst) = state.instruments.selected_instrument() {
                    let inst_id = inst.id;
                    // Auto-activate AudioIn instrument on start
                    if inst.source.is_audio_input() && !inst.active {
                        if let Some(inst_mut) = state.instruments.instrument_mut(inst_id) {
                            inst_mut.active = true;
                        }
                        let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                    let path = recording_path(&format!("input_{}", inst_id));
                    // Bus 0 is hardware out; for instrument recording we use bus 0
                    // since instruments route through output to bus 0
                    match audio_engine.start_recording(0, &path) {
                        Ok(()) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(
                                    audio_engine.status(),
                                    &format!("Recording to {}", path.display()),
                                );
                            }
                        }
                        Err(e) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(audio_engine.status(), &format!("Record error: {}", e));
                            }
                        }
                    }
                }
            }
        }
        ServerAction::Restart => {
            // Get selected devices before stopping
            let (input_dev, output_dev) = panes.get_pane_mut::<ServerPane>("server")
                .map(|s| (s.selected_input_device(), s.selected_output_device()))
                .unwrap_or((None, None));

            // Stop
            audio_engine.stop_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Stopped, "Restarting server...");
                server.set_server_running(false);
            }

            // Start with selected devices
            let start_result = audio_engine.start_server_with_devices(
                input_dev.as_deref(),
                output_dev.as_deref(),
            );
            match start_result {
                Ok(()) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Running, "Server restarted, connecting...");
                        server.set_server_running(true);
                    }

                    // Connect
                    let connect_result = audio_engine.connect("127.0.0.1:57110");
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        match connect_result {
                            Ok(()) => {
                                // Load built-in synthdefs
                                let synthdef_dir = std::path::Path::new("synthdefs");
                                let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

                                // Load custom synthdefs
                                let config_dir = config_synthdefs_dir();
                                let custom_result = if config_dir.exists() {
                                    audio_engine.load_synthdefs(&config_dir)
                                } else {
                                    Ok(())
                                };

                                // Load drum samples
                                for instrument in &state.instruments.instruments {
                                    if let Some(seq) = &instrument.drum_sequencer {
                                        for pad in &seq.pads {
                                            if let Some(buffer_id) = pad.buffer_id {
                                                if let Some(ref path) = pad.path {
                                                    let _ = audio_engine.load_sample(buffer_id, path);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Rebuild instrument routing
                                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);

                                match (builtin_result, custom_result) {
                                    (Ok(()), Ok(())) => {
                                        server.set_status(audio::ServerStatus::Connected, "Server restarted");
                                    }
                                    (Err(e), _) | (_, Err(e)) => {
                                        server.set_status(
                                            audio::ServerStatus::Connected,
                                            &format!("Restarted (synthdef warning: {})", e),
                                        );
                                    }
                                }
                                server.clear_device_config_dirty();
                            }
                            Err(e) => {
                                server.set_status(audio::ServerStatus::Error, &e.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Error, &e);
                        server.set_server_running(false);
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
            if let Err(e) = crate::state::persistence::save_project(&path, &state.session, &state.instruments) {
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
                    Ok((loaded_session, loaded_instruments)) => {
                        state.session = loaded_session;
                        state.instruments = loaded_instruments;
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
            if !panes.pop(&*state) {
                panes.switch_to("instrument", &*state);
            }
        }
        SessionAction::UpdateSessionLive(ref settings) => {
            state.session.apply_musical_settings(settings);
            state.session.piano_roll.time_signature = state.session.time_signature;
            state.session.piano_roll.bpm = state.session.bpm as f32;
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
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(step) = seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .and_then(|s| s.get_mut(*step_idx))
                {
                    step.active = !step.active;
                }
            }
        }
        SequencerAction::AdjustVelocity(pad_idx, step_idx, delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(step) = seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .and_then(|s| s.get_mut(*step_idx))
                {
                    step.velocity = (step.velocity as i16 + *delta as i16).clamp(1, 127) as u8;
                }
            }
        }
        SequencerAction::ClearPad(pad_idx) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                for step in seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .iter_mut()
                    .flat_map(|s| s.iter_mut())
                {
                    step.active = false;
                }
            }
        }
        SequencerAction::ClearPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let len = seq.pattern().length;
                *seq.pattern_mut() = DrumPattern::new(len);
            }
        }
        SequencerAction::CyclePatternLength => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let lengths = [8, 16, 32, 64];
                let current = seq.pattern().length;
                let idx = lengths.iter().position(|&l| l == current).unwrap_or(0);
                let new_len = lengths[(idx + 1) % lengths.len()];
                let old_pattern = seq.pattern().clone();
                let mut new_pattern = DrumPattern::new(new_len);
                for (pad_idx, old_steps) in old_pattern.steps.iter().enumerate() {
                    for (step_idx, step) in old_steps.iter().enumerate() {
                        if step_idx < new_len {
                            new_pattern.steps[pad_idx][step_idx] = step.clone();
                        }
                    }
                }
                *seq.pattern_mut() = new_pattern;
            }
        }
        SequencerAction::NextPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.current_pattern = (seq.current_pattern + 1) % seq.patterns.len();
            }
        }
        SequencerAction::PrevPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.current_pattern = if seq.current_pattern == 0 {
                    seq.patterns.len() - 1
                } else {
                    seq.current_pattern - 1
                };
            }
        }
        SequencerAction::AdjustPadLevel(pad_idx, delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                    pad.level = (pad.level + delta).clamp(0.0, 1.0);
                }
            }
        }
        SequencerAction::PlayStop => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.playing = !seq.playing;
                if !seq.playing {
                    seq.current_step = 0;
                    seq.step_accumulator = 0.0;
                }
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

            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
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
            }

            panes.pop(&*state);
        }
    }
}

fn dispatch_chopper(
    action: &ChopperAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        ChopperAction::LoadSample => {
            if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                fb.open_for(crate::ui::FileSelectAction::LoadChopperSample, None);
            }
            panes.push_to("file_browser", &*state);
        }
        ChopperAction::LoadSampleResult(path) => {
            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            // Compute waveform peaks from WAV file
            let (peaks, duration_secs) = compute_waveform_peaks(&path_str);

            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let buffer_id = seq.next_buffer_id;
                seq.next_buffer_id += 1;

                if audio_engine.is_running() {
                    let _ = audio_engine.load_sample(buffer_id, &path_str);
                }

                let initial_slice = Slice::full(0);
                seq.chopper = Some(ChopperState {
                    buffer_id: Some(buffer_id),
                    path: Some(path_str),
                    name,
                    slices: vec![initial_slice],
                    selected_slice: 0,
                    next_slice_id: 1,
                    waveform_peaks: peaks,
                    duration_secs,
                });
            }

            // Only pop if we're at the standalone file browser (pushed via LoadSample action).
            // When using the embedded browser inside the chopper pane, we're already where we want to be.
            if panes.active().id() == "file_browser" {
                panes.pop(&*state);
            }
        }
        ChopperAction::AddSlice(cursor_pos) => {
            let cursor_pos = *cursor_pos;
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    // Find which slice contains cursor_pos
                    if let Some(idx) = chopper.slices.iter().position(|s| s.start <= cursor_pos && s.end > cursor_pos) {
                        let old_end = chopper.slices[idx].end;
                        chopper.slices[idx].end = cursor_pos;

                        let new_id = chopper.next_slice_id;
                        chopper.next_slice_id += 1;
                        let new_slice = Slice::new(new_id, cursor_pos, old_end);
                        chopper.slices.insert(idx + 1, new_slice);
                        chopper.selected_slice = idx + 1;
                    }
                }
            }
        }
        ChopperAction::RemoveSlice => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if chopper.slices.len() > 1 {
                        let idx = chopper.selected_slice;
                        let removed = chopper.slices.remove(idx);
                        if idx > 0 {
                            // Extend previous slice's end to cover gap
                            chopper.slices[idx - 1].end = removed.end;
                            chopper.selected_slice = idx - 1;
                        } else if !chopper.slices.is_empty() {
                            // Extend next slice's start to cover gap
                            chopper.slices[0].start = removed.start;
                            chopper.selected_slice = 0;
                        }
                    }
                }
            }
        }
        ChopperAction::AssignToPad(pad_idx) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let assign_data = seq.chopper.as_ref().and_then(|c| {
                    c.slices.get(c.selected_slice).map(|s| (c.buffer_id, s.start, s.end))
                });
                if let Some((buffer_id, start, end)) = assign_data {
                    if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                        pad.buffer_id = buffer_id;
                        pad.slice_start = start;
                        pad.slice_end = end;
                        // Copy name from chopper
                        if let Some(chopper) = &seq.chopper {
                            pad.name = format!("{} {}", chopper.name, chopper.selected_slice + 1);
                            pad.path = chopper.path.clone();
                        }
                    }
                }
            }
        }
        ChopperAction::AutoSlice(n) => {
            let n = *n;
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    chopper.slices.clear();
                    for i in 0..n {
                        let start = i as f32 / n as f32;
                        let end = (i + 1) as f32 / n as f32;
                        let id = chopper.next_slice_id;
                        chopper.next_slice_id += 1;
                        chopper.slices.push(Slice::new(id, start, end));
                    }
                    chopper.selected_slice = 0;
                }
            }
        }
        ChopperAction::PreviewSlice => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                if let Some(seq) = &instrument.drum_sequencer {
                    if let Some(chopper) = &seq.chopper {
                        if let Some(slice) = chopper.slices.get(chopper.selected_slice) {
                            if let Some(buffer_id) = chopper.buffer_id {
                                if audio_engine.is_running() {
                                    let _ = audio_engine.play_drum_hit_to_instrument(
                                        buffer_id, 0.8, instrument.id,
                                        slice.start, slice.end,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        ChopperAction::SelectSlice(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if !chopper.slices.is_empty() {
                        let len = chopper.slices.len() as i8;
                        let new_idx = (chopper.selected_slice as i8 + delta).rem_euclid(len) as usize;
                        chopper.selected_slice = new_idx;
                    }
                }
            }
        }
        ChopperAction::NudgeSliceStart(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if let Some(slice) = chopper.slices.get_mut(chopper.selected_slice) {
                        slice.start = (slice.start + delta).clamp(0.0, slice.end - 0.001);
                    }
                }
            }
        }
        ChopperAction::NudgeSliceEnd(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if let Some(slice) = chopper.slices.get_mut(chopper.selected_slice) {
                        slice.end = (slice.end + delta).clamp(slice.start + 0.001, 1.0);
                    }
                }
            }
        }
        ChopperAction::CommitAll => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &seq.chopper {
                    let assignments: Vec<_> = chopper.slices.iter().enumerate()
                        .take(crate::state::drum_sequencer::NUM_PADS)
                        .map(|(i, s)| (i, chopper.buffer_id, s.start, s.end, chopper.name.clone(), chopper.path.clone()))
                        .collect();
                    for (i, buffer_id, start, end, name, path) in assignments {
                        if let Some(pad) = seq.pads.get_mut(i) {
                            pad.buffer_id = buffer_id;
                            pad.slice_start = start;
                            pad.slice_end = end;
                            pad.name = format!("{} {}", name, i + 1);
                            pad.path = path;
                        }
                    }
                }
            }
            panes.pop(&*state);
        }
        ChopperAction::MoveCursor(_) => {
            // Cursor tracked locally in pane
        }
    }
}

/// Compute waveform peaks from a WAV file for display
pub fn compute_waveform_peaks(path: &str) -> (Vec<f32>, f32) {
    let reader = match hound::WavReader::open(path) {
        Ok(r) => r,
        Err(_) => return (Vec::new(), 0.0),
    };
    let spec = reader.spec();
    let num_channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;
    let num_samples = reader.len() as usize;
    let duration_secs = num_samples as f32 / (sample_rate as f32 * num_channels as f32);

    let target_peaks = 512;
    let samples_per_peak = (num_samples / target_peaks).max(1);

    let mut peaks = Vec::with_capacity(target_peaks);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };

    for chunk in samples.chunks(samples_per_peak) {
        let peak = chunk.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
        peaks.push(peak);
    }

    (peaks, duration_secs)
}

/// Get the config directory for custom synthdefs
fn config_synthdefs_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("ilex")
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
    let temp_script = std::env::temp_dir().join("ilex_compile_custom.scd");
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
