mod audio;

mod dispatch;
mod midi;
mod panes;
mod playback;
mod scd_parser;
mod setup;
mod state;
mod ui;

use std::time::{Duration, Instant};

use audio::AudioEngine;
use panes::{AddPane, FileBrowserPane, FrameEditPane, HelpPane, HomePane, InstrumentEditPane, InstrumentPane, LogoPane, MixerPane, PianoRollPane, SampleChopperPane, SequencerPane, ServerPane};
use state::AppState;
use ui::{
    Action, Frame, InputSource, Keymap, PaneManager, RatatuiBackend, SessionAction, ViewState,
    keybindings,
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

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let mut state = AppState::new();

    // Load keybindings from embedded JSON (with optional user override)
    let (global_bindings, mut keymaps) = keybindings::load_keybindings();

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

    let mut audio_engine = AudioEngine::new();
    let mut app_frame = Frame::new();
    let mut last_frame_time = Instant::now();
    let mut active_notes: Vec<(u32, u8, u32)> = Vec::new();

    setup::auto_start_sc(&mut audio_engine, &state, &mut panes, &mut app_frame);

    loop {
        if let Some(event) = backend.poll_event(Duration::from_millis(16)) {
            let exclusive = panes.active().wants_exclusive_input();

            // Check global bindings (always_active ones work even in exclusive mode)
            if let Some(action) = global_bindings.lookup(&event, exclusive) {
                let handled = handle_global_action(
                    action,
                    &mut state,
                    &mut panes,
                    &mut audio_engine,
                    &mut app_frame,
                    &mut active_notes,
                );
                match handled {
                    GlobalResult::Quit => break,
                    GlobalResult::Handled => continue,
                    GlobalResult::NotHandled => {}
                }
            }

            let action = panes.handle_input(event, &state);
            if dispatch::dispatch_action(&action, &mut state, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes) {
                break;
            }
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

        // Piano roll playback tick
        {
            let now = Instant::now();
            let elapsed = now.duration_since(last_frame_time);
            last_frame_time = now;
            playback::tick_playback(&mut state, &mut audio_engine, &mut active_notes, elapsed);
            playback::tick_drum_sequencer(&mut state, &mut audio_engine, elapsed);
        }

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

        // Update waveform cache for piano roll
        if panes.active().id() == "piano_roll" {
            let track = panes.get_pane_mut::<PianoRollPane>("piano_roll")
                .map(|p| p.current_track()).unwrap_or(0);
            state.audio_in_waveform = state.session.piano_roll
                .track_at(track)
                .and_then(|t| state.instruments.instrument(t.module_id))
                .filter(|s| s.source.is_audio_input())
                .map(|s| audio_engine.audio_in_waveform(s.id));
        } else {
            state.audio_in_waveform = None;
        }

        // Render
        let mut frame = backend.begin_frame()?;
        let area = frame.area();
        app_frame.render_buf(area, frame.buffer_mut(), &state.session);
        panes.render(area, frame.buffer_mut(), &state);
        backend.end_frame(frame)?;
    }

    Ok(())
}

enum GlobalResult {
    Quit,
    Handled,
    NotHandled,
}

fn handle_global_action(
    action: &str,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
    app_frame: &mut Frame,
    active_notes: &mut Vec<(u32, u8, u32)>,
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
    let switch_to_pane = |target: &str, panes: &mut PaneManager, state: &mut AppState, app_frame: &mut Frame| {
        let current = capture_view(panes, state);
        app_frame.back_view = Some(current);
        app_frame.forward_view = None;
        panes.switch_to(target, &*state);
    };

    match action {
        "quit" => return GlobalResult::Quit,
        "save" => {
            dispatch::dispatch_action(&Action::Session(SessionAction::Save), state, panes, audio_engine, app_frame, active_notes);
        }
        "load" => {
            dispatch::dispatch_action(&Action::Session(SessionAction::Load), state, panes, audio_engine, app_frame, active_notes);
        }
        "master_mute" => {
            state.session.master_mute = !state.session.master_mute;
            if audio_engine.is_running() {
                let _ = audio_engine.update_all_instrument_mixer_params(&state.instruments, &state.session);
            }
        }
        "switch:instrument" => {
            switch_to_pane("instrument", panes, state, app_frame);
        }
        "switch:piano_roll_or_sequencer" => {
            let target = if state.instruments.selected_instrument()
                .map_or(false, |s| s.source.is_kit())
            {
                "sequencer"
            } else {
                "piano_roll"
            };
            switch_to_pane(target, panes, state, app_frame);
        }
        "switch:mixer" => {
            switch_to_pane("mixer", panes, state, app_frame);
        }
        "switch:server" => {
            switch_to_pane("server", panes, state, app_frame);
        }
        "switch:logo" => {
            switch_to_pane("logo", panes, state, app_frame);
        }
        "nav_back" => {
            if let Some(back) = app_frame.back_view.take() {
                let current = capture_view(panes, state);
                app_frame.forward_view = Some(current);
                restore_view(panes, state, &back);
            }
        }
        "nav_forward" => {
            if let Some(forward) = app_frame.forward_view.take() {
                let current = capture_view(panes, state);
                app_frame.back_view = Some(current);
                restore_view(panes, state, &forward);
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
                    _ => current_id,
                };
                if let Some(help) = panes.get_pane_mut::<HelpPane>("help") {
                    help.set_context(current_id, title, &current_keymap);
                }
                panes.push_to("help", &*state);
            }
        }
        _ => return GlobalResult::NotHandled,
    }
    GlobalResult::Handled
}
