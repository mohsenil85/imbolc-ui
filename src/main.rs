// Re-export core crate modules so crate::state, crate::audio, etc. resolve throughout the binary
pub use imbolc_core::action;
pub use imbolc_core::audio;
pub use imbolc_core::config;
pub use imbolc_core::dispatch;
pub use imbolc_core::midi;
pub use imbolc_core::scd_parser;
pub use imbolc_core::state;

mod panes;
mod setup;
mod ui;
mod global_actions;
mod midi_dispatch;

use std::time::{Duration, Instant};

use audio::AudioHandle;
use audio::commands::AudioCmd;
use action::{AudioDirty, IoFeedback};
use panes::{AddEffectPane, AddPane, AutomationPane, CommandPalettePane, ConfirmPane, EqPane, FileBrowserPane, FrameEditPane, HelpPane, HomePane, InstrumentEditPane, InstrumentPane, MidiSettingsPane, MixerPane, PianoRollPane, ProjectBrowserPane, SaveAsPane, SampleChopperPane, SequencerPane, ServerPane, TrackPane, VstParamPane, WaveformPane};
use state::AppState;
use ui::{
    Action, AppEvent, Frame, InputSource, KeyCode, Keymap, LayerResult,
    LayerStack, PaneManager, RatatuiBackend, keybindings,
};
use global_actions::*;

fn main() -> std::io::Result<()> {
    let mut backend = RatatuiBackend::new()?;
    backend.start()?;

    let result = run(&mut backend);

    backend.stop()?;
    result
}

fn pane_keymap(keymaps: &mut std::collections::HashMap<String, Keymap>, id: &str) -> Keymap {
    keymaps.remove(id).unwrap_or_else(Keymap::new)
}

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let (io_tx, io_rx) = std::sync::mpsc::channel::<IoFeedback>();
    let config = config::Config::load();
    let mut state = AppState::new_with_defaults(config.defaults());
    state.keyboard_layout = config.keyboard_layout();

    // Load keybindings from embedded TOML (with optional user override)
    let (layers, mut keymaps) = keybindings::load_keybindings();

    // file_browser keymap is used by both FileBrowserPane and SampleChopperPane's internal browser
    let file_browser_km = keymaps.get("file_browser").cloned().unwrap_or_else(Keymap::new);

    let mut panes = PaneManager::new(Box::new(InstrumentEditPane::new(pane_keymap(&mut keymaps, "instrument_edit"))));
    panes.add_pane(Box::new(HomePane::new(pane_keymap(&mut keymaps, "home"))));
    panes.add_pane(Box::new(AddPane::new(pane_keymap(&mut keymaps, "add"))));
    panes.add_pane(Box::new(InstrumentPane::new(pane_keymap(&mut keymaps, "instrument"))));
    panes.add_pane(Box::new(ServerPane::new(pane_keymap(&mut keymaps, "server"))));
    panes.add_pane(Box::new(MixerPane::new(pane_keymap(&mut keymaps, "mixer"))));
    panes.add_pane(Box::new(HelpPane::new(pane_keymap(&mut keymaps, "help"))));
    panes.add_pane(Box::new(PianoRollPane::new(pane_keymap(&mut keymaps, "piano_roll"))));
    panes.add_pane(Box::new(SequencerPane::new(pane_keymap(&mut keymaps, "sequencer"))));
    panes.add_pane(Box::new(FrameEditPane::new(pane_keymap(&mut keymaps, "frame_edit"))));
    panes.add_pane(Box::new(SampleChopperPane::new(pane_keymap(&mut keymaps, "sample_chopper"), file_browser_km)));
    panes.add_pane(Box::new(AddEffectPane::new(pane_keymap(&mut keymaps, "add_effect"))));
    panes.add_pane(Box::new(FileBrowserPane::new(pane_keymap(&mut keymaps, "file_browser"))));
    panes.add_pane(Box::new(TrackPane::new(pane_keymap(&mut keymaps, "track"))));
    panes.add_pane(Box::new(WaveformPane::new(pane_keymap(&mut keymaps, "waveform"))));
    panes.add_pane(Box::new(AutomationPane::new(pane_keymap(&mut keymaps, "automation"))));
    panes.add_pane(Box::new(EqPane::new(pane_keymap(&mut keymaps, "eq"))));
    panes.add_pane(Box::new(VstParamPane::new(pane_keymap(&mut keymaps, "vst_params"))));
    panes.add_pane(Box::new(ConfirmPane::new(pane_keymap(&mut keymaps, "confirm"))));
    panes.add_pane(Box::new(ProjectBrowserPane::new(pane_keymap(&mut keymaps, "project_browser"))));
    panes.add_pane(Box::new(SaveAsPane::new(pane_keymap(&mut keymaps, "save_as"))));
    panes.add_pane(Box::new(CommandPalettePane::new(pane_keymap(&mut keymaps, "command_palette"))));
    panes.add_pane(Box::new(MidiSettingsPane::new(pane_keymap(&mut keymaps, "midi_settings"))));

    // Create layer stack
    let mut layer_stack = LayerStack::new(layers);
    layer_stack.push("global");
    if state.instruments.instruments.is_empty() {
        panes.switch_to("add", &state);
    }
    layer_stack.set_pane_layer(panes.active().id());

    let mut audio = AudioHandle::new();
    audio.sync_state(&state);
    let mut app_frame = Frame::new();

    // Initialize MIDI input
    let mut midi_input = midi::MidiInputManager::new();
    midi_input.refresh_ports();
    // Auto-connect first available port
    if !midi_input.list_ports().is_empty() {
        let _ = midi_input.connect(0);
    }
    state.midi_port_names = midi_input.list_ports().iter().map(|p| p.name.clone()).collect();
    state.midi_connected_port = midi_input.connected_port_name().map(|s| s.to_string());
    let mut recent_projects = state::recent_projects::RecentProjects::load();
    let mut last_render_time = Instant::now();
    let mut select_mode = InstrumentSelectMode::Normal;
    let mut pending_audio_dirty = AudioDirty::default();

    // Auto-load most recent project on startup
    if let Some(entry) = recent_projects.entries.first() {
        let load_path = entry.path.clone();
        if load_path.exists() {
            if let Ok((session, instruments)) = state::persistence::load_project(&load_path) {
                let name = load_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("default")
                    .to_string();
                state.session = session;
                state.instruments = instruments;
                state.project_path = Some(load_path);
                state.dirty = false;
                app_frame.set_project_name(name);
                pending_audio_dirty.merge(AudioDirty::all());

                // Re-evaluate which pane to show
                if state.instruments.instruments.is_empty() {
                    panes.switch_to("add", &state);
                } else {
                    panes.switch_to("instrument_edit", &state);
                }
                layer_stack.set_pane_layer(panes.active().id());
            }
        }
    }

    // Auto-start SuperCollider and apply status events
    {
        let startup_events = setup::auto_start_sc(&mut audio);
        apply_status_events(&startup_events, &mut panes);
    }

    // Track last render area for mouse hit-testing
    let mut last_area = ratatui::layout::Rect::new(0, 0, 80, 24);

    loop {
        // Sync layer stack in case dispatch switched panes last iteration
        layer_stack.set_pane_layer(panes.active().id());

        if let Some(app_event) = backend.poll_event(Duration::from_millis(2)) {
            let pane_action = match app_event {
                AppEvent::Mouse(mouse_event) => {
                    panes.active_mut().handle_mouse(&mouse_event, last_area, &state)
                }
                AppEvent::Key(event) => {
                    // Two-digit instrument selection state machine (pre-layer)
                    match &select_mode {
                        InstrumentSelectMode::WaitingFirstDigit => {
                            if let KeyCode::Char(c) = event.key {
                                if let Some(d) = c.to_digit(10) {
                                    select_mode = InstrumentSelectMode::WaitingSecondDigit(d as u8);
                                    continue;
                                }
                            }
                            // Non-digit cancels
                            select_mode = InstrumentSelectMode::Normal;
                            // Fall through to normal handling
                        }
                        InstrumentSelectMode::WaitingSecondDigit(first) => {
                            let first = *first;
                            if let KeyCode::Char(c) = event.key {
                                if let Some(d) = c.to_digit(10) {
                                    let combined = first * 10 + d as u8;
                                    let target = if combined == 0 { 10 } else { combined };
                                    select_instrument(target as usize, &mut state, &mut panes, &mut audio, &io_tx);
                                    select_mode = InstrumentSelectMode::Normal;
                                    continue;
                                }
                            }
                            // Non-digit cancels
                            select_mode = InstrumentSelectMode::Normal;
                            // Fall through to normal handling
                        }
                        InstrumentSelectMode::Normal => {}
                    }

                    // Layer resolution
                    match layer_stack.resolve(&event) {
                        LayerResult::Action(action) => {
                            match handle_global_action(
                                action,
                                &mut state,
                                &mut panes,
                                &mut audio,
                                &mut app_frame,
                                &mut select_mode,
                                &mut pending_audio_dirty,
                                &mut layer_stack,
                                &io_tx,
                            ) {
                                GlobalResult::Quit => break,
                                GlobalResult::Handled => continue,
                                GlobalResult::NotHandled => {
                                    panes.active_mut().handle_action(action, &event, &state)
                                }
                            }
                        }
                        LayerResult::Blocked | LayerResult::Unresolved => {
                            panes.active_mut().handle_raw_input(&event, &state)
                        }
                    }
                }
            };

            // Process layer management actions
            match &pane_action {
                Action::PushLayer(name) => {
                    layer_stack.push(name);
                }
                Action::PopLayer(name) => {
                    layer_stack.pop(name);
                }
                Action::ExitPerformanceMode => {
                    layer_stack.pop("piano_mode");
                    layer_stack.pop("pad_mode");
                    panes.active_mut().deactivate_performance();
                }
                _ => {}
            }

            // Auto-pop text_edit layer when pane is no longer editing
            if layer_stack.has_layer("text_edit") {
                let still_editing = match panes.active().id() {
                    "instrument_edit" => {
                        panes.get_pane_mut::<InstrumentEditPane>("instrument_edit")
                            .map_or(false, |p| p.is_editing())
                    }
                    "frame_edit" => {
                        panes.get_pane_mut::<FrameEditPane>("frame_edit")
                            .map_or(false, |p| p.is_editing())
                    }
                    _ => false,
                };
                if !still_editing {
                    layer_stack.pop("text_edit");
                }
            }

            // Process navigation
            panes.process_nav(&pane_action, &state);

            // Sync pane layer after navigation
            if matches!(&pane_action, Action::Nav(_)) {
                sync_pane_layer(&mut panes, &mut layer_stack);

                // Auto-exit clip edit when navigating away from piano roll
                if state.session.arrangement.editing_clip.is_some()
                    && panes.active().id() != "piano_roll"
                {
                    let exit_result = dispatch::dispatch_action(
                        &Action::Arrangement(action::ArrangementAction::ExitClipEdit),
                        &mut state, &mut audio, &io_tx,
                    );
                    pending_audio_dirty.merge(exit_result.audio_dirty);
                    apply_dispatch_result(exit_result, &mut state, &mut panes, &mut app_frame, &mut audio);
                }
            }

            // Auto-pop command_palette layer and re-dispatch confirmed command
            if layer_stack.has_layer("command_palette") && panes.active().id() != "command_palette" {
                layer_stack.pop("command_palette");
                if let Some(palette) = panes.get_pane_mut::<CommandPalettePane>("command_palette") {
                    if let Some(cmd) = palette.take_command() {
                        let global_result = handle_global_action(
                            &cmd, &mut state, &mut panes, &mut audio, &mut app_frame,
                            &mut select_mode, &mut pending_audio_dirty, &mut layer_stack, &io_tx,
                        );
                        if matches!(global_result, GlobalResult::Quit) { break; }
                        if matches!(global_result, GlobalResult::NotHandled) {
                            let dummy_event = ui::InputEvent::new(KeyCode::Enter, ui::Modifiers::none());
                            let re_action = panes.active_mut().handle_action(&cmd, &dummy_event, &state);
                            panes.process_nav(&re_action, &state);
                            if matches!(&re_action, Action::Nav(_)) {
                                sync_pane_layer(&mut panes, &mut layer_stack);
                            }
                            let r = dispatch::dispatch_action(&re_action, &mut state, &mut audio, &io_tx);
                            if r.quit { break; }
                            pending_audio_dirty.merge(r.audio_dirty);
                            apply_dispatch_result(r, &mut state, &mut panes, &mut app_frame, &mut audio);
                        }
                    }
                }
                sync_pane_layer(&mut panes, &mut layer_stack);
            }

            // Intercept MIDI port actions that need MidiInputManager
            if let Action::Midi(action::MidiAction::ConnectPort(port_idx)) = &pane_action {
                let port_idx = *port_idx;
                midi_input.refresh_ports();
                match midi_input.connect(port_idx) {
                    Ok(()) => {
                        state.midi_connected_port = midi_input.connected_port_name().map(|s| s.to_string());
                    }
                    Err(_) => {
                        state.midi_connected_port = None;
                    }
                }
                state.midi_port_names = midi_input.list_ports().iter().map(|p| p.name.clone()).collect();
            } else if let Action::Midi(action::MidiAction::DisconnectPort) = &pane_action {
                midi_input.disconnect();
                state.midi_connected_port = None;
            }

            let dispatch_result = dispatch::dispatch_action(&pane_action, &mut state, &mut audio, &io_tx);
            if dispatch_result.quit {
                break;
            }
            pending_audio_dirty.merge(dispatch_result.audio_dirty);
            apply_dispatch_result(dispatch_result, &mut state, &mut panes, &mut app_frame, &mut audio);
        }

        if pending_audio_dirty.any() {
            audio.flush_dirty(&state, pending_audio_dirty);
            pending_audio_dirty.clear();
        }

        // Drain I/O feedback
        while let Ok(feedback) = io_rx.try_recv() {
            match feedback {
                IoFeedback::SaveComplete { id, path, result } => {
                    if id != state.io_generation.save {
                        continue;
                    }
                    let status = match result {
                        Ok(name) => {
                            state.project_path = Some(path.clone());
                            state.dirty = false;
                            recent_projects.add(&path, &name);
                            recent_projects.save();
                            app_frame.set_project_name(name);
                            "Saved project".to_string()
                        }
                        Err(e) => format!("Save failed: {}", e),
                    };
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio.status(), &status);
                    }
                }
                IoFeedback::LoadComplete { id, path, result } => {
                    if id != state.io_generation.load {
                        continue;
                    }
                     match result {
                         Ok((new_session, new_instruments, name)) => {
                             state.undo_history.clear();
                             state.session = new_session;
                             state.instruments = new_instruments;
                             state.project_path = Some(path.clone());
                             state.dirty = false;
                             recent_projects.add(&path, &name);
                             recent_projects.save();
                             app_frame.set_project_name(name);
                             
                             if state.instruments.instruments.is_empty() {
                                 panes.switch_to("add", &state);
                             }

                             let dirty = AudioDirty::all();
                             pending_audio_dirty.merge(dirty);
                             
                             // Queue VST state restores
                             for inst in &state.instruments.instruments {
                                if let (state::SourceType::Vst(_), Some(ref path)) = (&inst.source, &inst.vst_state_path) {
                                    let _ = audio.send_cmd(audio::commands::AudioCmd::LoadVstState {
                                        instrument_id: inst.id,
                                        target: action::VstTarget::Source,
                                        path: path.clone(),
                                    });
                                }
                                for effect in &inst.effects {
                                    if let (state::EffectType::Vst(_), Some(ref path)) = (&effect.effect_type, &effect.vst_state_path) {
                                        let _ = audio.send_cmd(audio::commands::AudioCmd::LoadVstState {
                                            instrument_id: inst.id,
                                            target: action::VstTarget::Effect(effect.id),
                                            path: path.clone(),
                                        });
                                    }
                                }
                             }

                             if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                 server.set_status(audio.status(), "Project loaded");
                             }
                         }
                         Err(e) => {
                             if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                 server.set_status(audio.status(), &format!("Load failed: {}", e));
                             }
                         }
                     }
                }
                IoFeedback::ImportSynthDefComplete { id, result } => {
                    if id != state.io_generation.import_synthdef {
                        continue;
                    }
                     match result {
                         Ok((custom, synthdef_name, scsyndef_path)) => {
                             // Register it
                             let _id = state.session.custom_synthdefs.add(custom);
                             pending_audio_dirty.session = true;

                             if audio.is_running() {
                                 if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                     server.set_status(audio.status(), &format!("Loading custom synthdef: {}", synthdef_name));
                                 }

                                 let (reply_tx, reply_rx) = std::sync::mpsc::channel();
                                 let load_path = scsyndef_path.clone();
                                 let io_tx = io_tx.clone();
                                 let load_id = id;
                                 let name = synthdef_name.clone();

                                 match audio.send_cmd(AudioCmd::LoadSynthDefFile { path: load_path, reply: reply_tx }) {
                                     Ok(()) => {
                                         std::thread::spawn(move || {
                                             let result = match reply_rx.recv() {
                                                 Ok(Ok(())) => Ok(name),
                                                 Ok(Err(e)) => Err(e),
                                                 Err(_) => Err("Audio thread disconnected".to_string()),
                                             };
                                             let _ = io_tx.send(IoFeedback::ImportSynthDefLoaded { id: load_id, result });
                                         });
                                     }
                                     Err(e) => {
                                         if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                             server.set_status(audio.status(), &format!("Failed to load synthdef: {}", e));
                                         }
                                     }
                                 }
                             } else {
                                 if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                     server.set_status(audio.status(), &format!("Imported custom synthdef: {}", synthdef_name));
                                 }
                             }
                         }
                         Err(e) => {
                             if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                 server.set_status(audio.status(), &format!("Import error: {}", e));
                             }
                         }
                     }
                }
                IoFeedback::ImportSynthDefLoaded { id, result } => {
                    if id != state.io_generation.import_synthdef {
                        continue;
                    }
                    let status = match result {
                        Ok(name) => format!("Loaded custom synthdef: {}", name),
                        Err(e) => format!("Failed to load synthdef: {}", e),
                    };
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio.status(), &status);
                    }
                }
            }
        }

        // Drain audio feedback
        for feedback in audio.drain_feedback() {
            let action = Action::AudioFeedback(feedback);
            let r = dispatch::dispatch_action(&action, &mut state, &mut audio, &io_tx);
            pending_audio_dirty.merge(r.audio_dirty);
            apply_dispatch_result(r, &mut state, &mut panes, &mut app_frame, &mut audio);
        }

        // Poll MIDI events
        for event in midi_input.poll_events() {
            if let Some(action) = midi_dispatch::process_midi_event(&event, &state) {
                let r = dispatch::dispatch_action(&action, &mut state, &mut audio, &io_tx);
                pending_audio_dirty.merge(r.audio_dirty);
            }
        }

        // Visual updates and rendering at ~60fps
        let now_render = Instant::now();
        if now_render.duration_since(last_render_time).as_millis() >= 16 {
            last_render_time = now_render;

            // Update master meter from real audio peak
            {
                let peak = if audio.is_running() {
                    audio.master_peak()
                } else {
                    0.0
                };
                let mute = state.session.master_mute;
                app_frame.set_master_peak(peak, mute);
            }

            // Update SC CPU and latency indicators
            {
                let cpu = if audio.is_running() { audio.sc_cpu() } else { 0.0 };
                let latency = if audio.is_running() { audio.osc_latency_ms() } else { 0.0 };
                app_frame.set_sc_metrics(cpu, latency);
            }

            // Update recording state
            state.recording = audio.is_recording();
            state.recording_secs = audio.recording_elapsed()
                .map(|d| d.as_secs()).unwrap_or(0);
            app_frame.recording = state.recording;
            app_frame.recording_secs = state.recording_secs;

            // Update visualization data from audio analysis synths
            state.visualization.spectrum_bands = audio.spectrum_bands();
            let (peak_l, peak_r, rms_l, rms_r) = audio.lufs_data();
            state.visualization.peak_l = peak_l;
            state.visualization.peak_r = peak_r;
            state.visualization.rms_l = rms_l;
            state.visualization.rms_r = rms_r;
            let scope = audio.scope_buffer();
            state.visualization.scope_buffer.clear();
            state.visualization.scope_buffer.extend(scope);

            // Update waveform cache for waveform pane
            if panes.active().id() == "waveform" {
                if let Some(wf) = panes.get_pane_mut::<WaveformPane>("waveform") {
                    if state.recorded_waveform_peaks.is_none() {
                        wf.audio_in_waveform = state.instruments.selected_instrument()
                            .filter(|s| s.source.is_audio_input() || s.source.is_bus_in())
                            .map(|s| audio.audio_in_waveform(s.id));
                    }
                }
            } else {
                if let Some(wf) = panes.get_pane_mut::<WaveformPane>("waveform") {
                    wf.audio_in_waveform = None;
                }
                state.recorded_waveform_peaks = None;
            }

            // Copy audio-owned state into AppState for pane rendering.
            {
                let ars = audio.read_state();
                state.audio_playhead = ars.playhead;
                state.audio_bpm = ars.bpm;
                state.server_status = ars.server_status;
            }

            // Render
            let mut frame = backend.begin_frame()?;
            let area = frame.area();
            last_area = area;
            let mut rbuf = ui::RenderBuf::new(frame.buffer_mut());
            app_frame.render_buf(area, &mut rbuf, &state);
            panes.render(area, &mut rbuf, &state);
            backend.end_frame(frame)?;
        }
    }

    Ok(())
}


