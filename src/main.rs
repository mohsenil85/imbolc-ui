// Re-export core crate modules so crate::state, crate::audio, etc. resolve throughout the binary
pub use ilex_core::action;
pub use ilex_core::audio;
pub use ilex_core::config;
pub use ilex_core::dispatch;
pub use ilex_core::midi;
pub use ilex_core::playback;
pub use ilex_core::scd_parser;
pub use ilex_core::state;

mod panes;
mod setup;
mod ui;

use std::time::{Duration, Instant};

use audio::AudioEngine;
use panes::{AddPane, AutomationPane, FileBrowserPane, FrameEditPane, HelpPane, HomePane, InstrumentEditPane, InstrumentPane, LogoPane, MixerPane, PianoRollPane, SampleChopperPane, SequencerPane, ServerPane, TrackPane, WaveformPane};
use state::AppState;
use ui::{
    Action, AppEvent, DispatchResult, Frame, InputSource, KeyCode, Keymap, LayerResult,
    LayerStack, NavIntent, PaneManager, RatatuiBackend, SessionAction, StatusEvent,
    ToggleResult, ViewState, keybindings,
};

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

/// Two-digit instrument selection state machine
enum InstrumentSelectMode {
    Normal,
    WaitingFirstDigit,
    WaitingSecondDigit(u8),
}

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let config = config::Config::load();
    let mut state = AppState::new_with_defaults(config.defaults());
    state.keyboard_layout = config.keyboard_layout();

    // Load keybindings from embedded TOML (with optional user override)
    let (layers, mut keymaps) = keybindings::load_keybindings();

    // file_browser keymap is used by both FileBrowserPane and SampleChopperPane's internal browser
    let file_browser_km = keymaps.get("file_browser").cloned().unwrap_or_else(Keymap::new);

    let mut panes = PaneManager::new(Box::new(InstrumentPane::new(pane_keymap(&mut keymaps, "instrument"))));
    panes.add_pane(Box::new(HomePane::new(pane_keymap(&mut keymaps, "home"))));
    panes.add_pane(Box::new(AddPane::new(pane_keymap(&mut keymaps, "add"))));
    panes.add_pane(Box::new(InstrumentEditPane::new(pane_keymap(&mut keymaps, "instrument_edit"))));
    panes.add_pane(Box::new(ServerPane::new(pane_keymap(&mut keymaps, "server"))));
    panes.add_pane(Box::new(MixerPane::new(pane_keymap(&mut keymaps, "mixer"))));
    panes.add_pane(Box::new(HelpPane::new(pane_keymap(&mut keymaps, "help"))));
    panes.add_pane(Box::new(PianoRollPane::new(pane_keymap(&mut keymaps, "piano_roll"))));
    panes.add_pane(Box::new(SequencerPane::new(pane_keymap(&mut keymaps, "sequencer"))));
    panes.add_pane(Box::new(FrameEditPane::new(pane_keymap(&mut keymaps, "frame_edit"))));
    panes.add_pane(Box::new(SampleChopperPane::new(pane_keymap(&mut keymaps, "sample_chopper"), file_browser_km)));
    panes.add_pane(Box::new(FileBrowserPane::new(pane_keymap(&mut keymaps, "file_browser"))));
    panes.add_pane(Box::new(LogoPane::new(pane_keymap(&mut keymaps, "logo"))));
    panes.add_pane(Box::new(TrackPane::new(pane_keymap(&mut keymaps, "track"))));
    panes.add_pane(Box::new(WaveformPane::new(pane_keymap(&mut keymaps, "waveform"))));
    panes.add_pane(Box::new(AutomationPane::new(pane_keymap(&mut keymaps, "automation"))));

    // Create layer stack
    let mut layer_stack = LayerStack::new(layers);
    layer_stack.push("global");
    layer_stack.set_pane_layer(panes.active().id());

    let mut audio_engine = AudioEngine::new();
    let mut app_frame = Frame::new();
    let mut last_frame_time = Instant::now();
    let mut last_render_time = Instant::now();
    let mut active_notes: Vec<(u32, u8, u32)> = Vec::new();
    let mut select_mode = InstrumentSelectMode::Normal;

    // Auto-start SuperCollider and apply status events
    {
        let startup_events = setup::auto_start_sc(&mut audio_engine, &state);
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
                                    select_instrument(target as usize, &mut state, &mut panes);
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
                                &mut audio_engine,
                                &mut app_frame,
                                &mut active_notes,
                                &mut select_mode,
                                &mut layer_stack,
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
            }

            let dispatch_result = dispatch::dispatch_action(&pane_action, &mut state, &mut audio_engine, &mut active_notes);
            if dispatch_result.quit {
                break;
            }
            apply_dispatch_result(dispatch_result, &mut state, &mut panes, &mut app_frame);
        }

        // Poll for background compile completion
        if let Some(result) = audio_engine.poll_compile_result() {
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(msg) => server.set_status(audio_engine.status(), &msg),
                    Err(e) => server.set_status(audio_engine.status(), &e),
                }
            }
        }

        // Check scsynth process health
        if let Some(msg) = audio_engine.check_server_health() {
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio_engine.status(), &msg);
                server.set_server_running(false);
            }
        }

        // Piano roll playback tick
        {
            let now = Instant::now();
            let elapsed = now.duration_since(last_frame_time);
            last_frame_time = now;
            playback::tick_playback(&mut state, &mut audio_engine, &mut active_notes, elapsed);
            playback::tick_drum_sequencer(&mut state, &mut audio_engine, elapsed);
        }

        // Deferred recording buffer free + waveform load
        // Wait for scsynth to flush the WAV file before reading it
        if audio_engine.poll_pending_buffer_free() {
            if let Some(path) = state.pending_recording_path.take() {
                let peaks = dispatch::compute_waveform_peaks(&path.to_string_lossy()).0;
                if !peaks.is_empty() {
                    if let Some(wf) = panes.get_pane_mut::<WaveformPane>("waveform") {
                        wf.recorded_waveform = Some(peaks);
                    }
                    panes.switch_to("waveform", &state);
                }
            }
        }

        // Visual updates and rendering at ~60fps
        let now_render = Instant::now();
        if now_render.duration_since(last_render_time).as_millis() >= 16 {
            last_render_time = now_render;

            // Update master meter from real audio peak
            {
                let peak = if audio_engine.is_running() {
                    audio_engine.master_peak()
                } else {
                    0.0
                };
                let mute = state.session.master_mute;
                app_frame.set_master_peak(peak, mute);
            }

            // Update recording state
            state.recording = audio_engine.is_recording();
            state.recording_secs = audio_engine.recording_elapsed()
                .map(|d| d.as_secs()).unwrap_or(0);
            app_frame.recording = state.recording;
            app_frame.recording_secs = state.recording_secs;

            // Update waveform cache for waveform pane
            if panes.active().id() == "waveform" {
                if let Some(wf) = panes.get_pane_mut::<WaveformPane>("waveform") {
                    if wf.recorded_waveform.is_none() {
                        wf.audio_in_waveform = state.instruments.selected_instrument()
                            .filter(|s| s.source.is_audio_input() || s.source.is_bus_in())
                            .map(|s| audio_engine.audio_in_waveform(s.id));
                    }
                }
            } else {
                if let Some(wf) = panes.get_pane_mut::<WaveformPane>("waveform") {
                    wf.audio_in_waveform = None;
                    wf.recorded_waveform = None;
                }
            }

            // Render
            let mut frame = backend.begin_frame()?;
            let area = frame.area();
            last_area = area;
            app_frame.render_buf(area, frame.buffer_mut(), &state);
            panes.render(area, frame.buffer_mut(), &state);
            backend.end_frame(frame)?;
        }
    }

    Ok(())
}

enum GlobalResult {
    Quit,
    Handled,
    NotHandled,
}

/// Select instrument by 1-based number (1=first, 10=tenth) and sync piano roll
fn select_instrument(number: usize, state: &mut AppState, panes: &mut PaneManager) {
    let idx = number.saturating_sub(1); // Convert 1-based to 0-based
    if idx < state.instruments.instruments.len() {
        state.instruments.selected = Some(idx);
        sync_piano_roll_to_selection(state, panes);
    }
}

/// Sync piano roll's current track to match the globally selected instrument,
/// and re-route the active pane if on a F2-family pane (piano_roll/sequencer/waveform).
fn sync_piano_roll_to_selection(state: &mut AppState, panes: &mut PaneManager) {
    if let Some(selected_idx) = state.instruments.selected {
        if let Some(inst) = state.instruments.instruments.get(selected_idx) {
            let inst_id = inst.id;
            // Find which track index corresponds to this instrument
            if let Some(track_idx) = state.session.piano_roll.track_order.iter()
                .position(|&id| id == inst_id)
            {
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.set_current_track(track_idx);
                }
            }

            // Sync mixer selection
            let active = panes.active().id();
            if active == "mixer" {
                if let state::MixerSelection::Instrument(_) = state.session.mixer_selection {
                    state.session.mixer_selection = state::MixerSelection::Instrument(selected_idx);
                }
            }

            // Re-route if currently on a F2-family pane
            if active == "piano_roll" || active == "sequencer" || active == "waveform" {
                let target = if inst.source.is_kit() {
                    "sequencer"
                } else if inst.source.is_audio_input() || inst.source.is_bus_in() {
                    "waveform"
                } else {
                    "piano_roll"
                };
                if active != target {
                    panes.switch_to(target, state);
                }
            }
        }
    }
}

/// Sync layer stack pane layer and performance mode state after pane switch.
fn sync_pane_layer(panes: &mut PaneManager, layer_stack: &mut LayerStack) {
    let had_piano = layer_stack.has_layer("piano_mode");
    let had_pad = layer_stack.has_layer("pad_mode");
    layer_stack.set_pane_layer(panes.active().id());
    if had_piano {
        panes.active_mut().activate_piano();
    }
    if had_pad {
        panes.active_mut().activate_pad();
    }
}

fn handle_global_action(
    action: &str,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    app_frame: &mut Frame,
    active_notes: &mut Vec<(u32, u8, u32)>,
    select_mode: &mut InstrumentSelectMode,
    layer_stack: &mut LayerStack,
) -> GlobalResult {
    // Helper to capture current view state
    let capture_view = |panes: &mut PaneManager, state: &AppState| -> ViewState {
        let pane_id = panes.active().id().to_string();
        let inst_selection = state.instruments.selected;
        let edit_tab = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit")
            .map(|ep| ep.tab_index())
            .unwrap_or(0);
        ViewState { pane_id, inst_selection, edit_tab }
    };

    // Helper to restore view state
    let restore_view = |panes: &mut PaneManager, state: &mut AppState, view: &ViewState| {
        state.instruments.selected = view.inst_selection;
        if let Some(edit_pane) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
            edit_pane.set_tab_index(view.edit_tab);
        }
        panes.switch_to(&view.pane_id, &*state);
    };

    // Helper for pane switching with view history
    let switch_to_pane = |target: &str, panes: &mut PaneManager, state: &mut AppState, app_frame: &mut Frame, layer_stack: &mut LayerStack| {
        let current = capture_view(panes, state);
        if app_frame.view_history.is_empty() {
            app_frame.view_history.push(current);
        } else {
            app_frame.view_history[app_frame.history_cursor] = current;
        }
        // Truncate forward history
        app_frame.view_history.truncate(app_frame.history_cursor + 1);
        // Switch and record new view
        panes.switch_to(target, &*state);
        sync_pane_layer(panes, layer_stack);
        let new_view = capture_view(panes, state);
        app_frame.view_history.push(new_view);
        app_frame.history_cursor = app_frame.view_history.len() - 1;
    };

    match action {
        "quit" => return GlobalResult::Quit,
        "save" => {
            let r = dispatch::dispatch_action(&Action::Session(SessionAction::Save), state, audio_engine, active_notes);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "load" => {
            let r = dispatch::dispatch_action(&Action::Session(SessionAction::Load), state, audio_engine, active_notes);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "master_mute" => {
            state.session.master_mute = !state.session.master_mute;
            if audio_engine.is_running() {
                let _ = audio_engine.update_all_instrument_mixer_params(&state.instruments, &state.session);
            }
        }
        "record_master" => {
            let r = dispatch::dispatch_action(&Action::Server(ui::ServerAction::RecordMaster), state, audio_engine, active_notes);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "switch:instrument" => {
            switch_to_pane("instrument", panes, state, app_frame, layer_stack);
        }
        "switch:piano_roll_or_sequencer" => {
            let target = if let Some(inst) = state.instruments.selected_instrument() {
                if inst.source.is_kit() {
                    "sequencer"
                } else if inst.source.is_audio_input() || inst.source.is_bus_in() {
                    "waveform"
                } else {
                    "piano_roll"
                }
            } else {
                "piano_roll"
            };
            switch_to_pane(target, panes, state, app_frame, layer_stack);
        }
        "switch:track" => {
            switch_to_pane("track", panes, state, app_frame, layer_stack);
        }
        "switch:mixer" => {
            switch_to_pane("mixer", panes, state, app_frame, layer_stack);
        }
        "switch:server" => {
            switch_to_pane("server", panes, state, app_frame, layer_stack);
        }
        "switch:logo" => {
            switch_to_pane("logo", panes, state, app_frame, layer_stack);
        }
        "switch:automation" => {
            switch_to_pane("automation", panes, state, app_frame, layer_stack);
        }
        "switch:frame_edit" => {
            if panes.active().id() == "frame_edit" {
                panes.pop(&*state);
            } else {
                panes.push_to("frame_edit", &*state);
            }
        }
        "nav_back" => {
            let history = &mut app_frame.view_history;
            if !history.is_empty() {
                let current = capture_view(panes, state);
                history[app_frame.history_cursor] = current;

                let at_front = app_frame.history_cursor == history.len() - 1;
                if at_front {
                    if app_frame.history_cursor > 0 {
                        app_frame.history_cursor -= 1;
                        let view = history[app_frame.history_cursor].clone();
                        restore_view(panes, state, &view);
                        sync_pane_layer(panes, layer_stack);
                    }
                } else {
                    if app_frame.history_cursor < history.len() - 1 {
                        app_frame.history_cursor += 1;
                        let view = history[app_frame.history_cursor].clone();
                        restore_view(panes, state, &view);
                        sync_pane_layer(panes, layer_stack);
                    }
                }
            }
        }
        "nav_forward" => {
            let history = &mut app_frame.view_history;
            if !history.is_empty() {
                let current = capture_view(panes, state);
                history[app_frame.history_cursor] = current;

                let at_front = app_frame.history_cursor == history.len() - 1;
                if at_front {
                    let target = app_frame.history_cursor.saturating_sub(2);
                    if target != app_frame.history_cursor {
                        app_frame.history_cursor = target;
                        let view = history[app_frame.history_cursor].clone();
                        restore_view(panes, state, &view);
                        sync_pane_layer(panes, layer_stack);
                    }
                } else {
                    let target = (app_frame.history_cursor + 2).min(history.len() - 1);
                    if target != app_frame.history_cursor {
                        app_frame.history_cursor = target;
                        let view = history[app_frame.history_cursor].clone();
                        restore_view(panes, state, &view);
                        sync_pane_layer(panes, layer_stack);
                    }
                }
            }
        }
        "help" => {
            if panes.active().id() != "help" {
                let current_id = panes.active().id();
                let current_keymap = panes.active().keymap().clone();
                let title = match current_id {
                    "instrument" => "Instruments",
                    "mixer" => "Mixer",
                    "server" => "Server",
                    "piano_roll" => "Piano Roll",
                    "sequencer" => "Step Sequencer",
                    "add" => "Add Instrument",
                    "instrument_edit" => "Edit Instrument",
                    "track" => "Track",
                    "waveform" => "Waveform",
                    "automation" => "Automation",
                    _ => current_id,
                };
                if let Some(help) = panes.get_pane_mut::<HelpPane>("help") {
                    help.set_context(current_id, title, &current_keymap);
                }
                panes.push_to("help", &*state);
            }
        }
        // Instrument selection by number (1-9 select instruments 1-9, 0 selects 10)
        s if s.starts_with("select:") => {
            if let Ok(n) = s[7..].parse::<usize>() {
                select_instrument(n, state, panes);
            }
        }
        "select_prev_instrument" => {
            state.instruments.select_prev();
            sync_piano_roll_to_selection(state, panes);
        }
        "select_next_instrument" => {
            state.instruments.select_next();
            sync_piano_roll_to_selection(state, panes);
        }
        "select_two_digit" => {
            *select_mode = InstrumentSelectMode::WaitingFirstDigit;
        }
        "toggle_piano_mode" => {
            let result = panes.active_mut().toggle_performance_mode(state);
            match result {
                ToggleResult::ActivatedPiano => {
                    layer_stack.push("piano_mode");
                }
                ToggleResult::ActivatedPad => {
                    layer_stack.push("pad_mode");
                }
                ToggleResult::Deactivated => {
                    layer_stack.pop("piano_mode");
                    layer_stack.pop("pad_mode");
                }
                ToggleResult::CycledLayout | ToggleResult::NotSupported => {}
            }
        }
        "escape" => {
            // Global escape — falls through to pane when no mode layer handles it
            return GlobalResult::NotHandled;
        }
        _ => return GlobalResult::NotHandled,
    }
    GlobalResult::Handled
}

/// Apply status events from dispatch or setup to the server pane
fn apply_status_events(events: &[StatusEvent], panes: &mut PaneManager) {
    for event in events {
        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
            server.set_status(event.status, &event.message);
            if let Some(running) = event.server_running {
                server.set_server_running(running);
            }
        }
    }
}

/// Apply a DispatchResult to the UI layer: process nav intents, status events, project name
fn apply_dispatch_result(
    result: DispatchResult,
    state: &mut AppState,
    panes: &mut PaneManager,
    app_frame: &mut Frame,
) {
    // Process nav intents
    for intent in &result.nav {
        match intent {
            NavIntent::OpenFileBrowser(file_action) => {
                if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                    fb.open_for(file_action.clone(), None);
                }
                panes.push_to("file_browser", state);
            }
            _ => {}
        }
    }
    panes.process_nav_intents(&result.nav, state);

    // Process status events
    apply_status_events(&result.status, panes);

    // Process project name
    if let Some(ref name) = result.project_name {
        app_frame.set_project_name(name.to_string());
    }
}
