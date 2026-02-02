use crate::audio::AudioHandle;
use crate::action::{AudioDirty, IoFeedback};
use crate::dispatch;
use crate::state::{self, AppState};
use crate::panes::{InstrumentEditPane, PianoRollPane, ServerPane, HelpPane, FileBrowserPane, VstParamPane};
use crate::ui::{
    self, Action, DispatchResult, Frame, LayerStack, NavIntent, PaneManager,
    SessionAction, StatusEvent, ToggleResult, ViewState
};

/// Two-digit instrument selection state machine
pub(crate) enum InstrumentSelectMode {
    Normal,
    WaitingFirstDigit,
    WaitingSecondDigit(u8),
}

pub(crate) enum GlobalResult {
    Quit,
    Handled,
    NotHandled,
}

/// Select instrument by 1-based number (1=first, 10=tenth) and sync piano roll
pub(crate) fn select_instrument(number: usize, state: &mut AppState, panes: &mut PaneManager) {
    let idx = number.saturating_sub(1); // Convert 1-based to 0-based
    if idx < state.instruments.instruments.len() {
        state.instruments.selected = Some(idx);
        sync_piano_roll_to_selection(state, panes);
        sync_instrument_edit(state, panes);
    }
}

/// Sync piano roll's current track to match the globally selected instrument,
/// and re-route the active pane if on a F2-family pane (piano_roll/sequencer/waveform).
pub(crate) fn sync_piano_roll_to_selection(state: &mut AppState, panes: &mut PaneManager) {
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

/// If the instrument edit pane is active, reload it with the currently selected instrument.
pub(crate) fn sync_instrument_edit(state: &AppState, panes: &mut PaneManager) {
    if panes.active().id() == "instrument_edit" {
        if let Some(inst) = state.instruments.selected_instrument() {
            if let Some(edit_pane) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
                edit_pane.set_instrument(inst);
            }
        }
    }
}

/// Sync layer stack pane layer and performance mode state after pane switch.
pub(crate) fn sync_pane_layer(panes: &mut PaneManager, layer_stack: &mut LayerStack) {
    let had_piano = layer_stack.has_layer("piano_mode");
    let had_pad = layer_stack.has_layer("pad_mode");
    layer_stack.set_pane_layer(panes.active().id());

    if had_piano || had_pad {
        if panes.active_mut().supports_performance_mode() {
            if had_piano { panes.active_mut().activate_piano(); }
            if had_pad { panes.active_mut().activate_pad(); }
        } else {
            layer_stack.pop("piano_mode");
            layer_stack.pop("pad_mode");
            panes.active_mut().deactivate_performance();
        }
    }
}

pub(crate) fn handle_global_action(
    action: &str,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio: &mut AudioHandle,
    app_frame: &mut Frame,
    select_mode: &mut InstrumentSelectMode,
    pending_audio_dirty: &mut AudioDirty,
    layer_stack: &mut LayerStack,
    io_tx: &std::sync::mpsc::Sender<IoFeedback>,
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
        // Sync mixer highlight to global instrument selection on entry
        if target == "mixer" {
            if let Some(selected_idx) = state.instruments.selected {
                state.session.mixer_selection = state::MixerSelection::Instrument(selected_idx);
            }
        }
        let new_view = capture_view(panes, state);
        app_frame.view_history.push(new_view);
        app_frame.history_cursor = app_frame.view_history.len() - 1;
    };

    match action {
        "quit" => return GlobalResult::Quit,
        "save" => {
            let r = dispatch::dispatch_action(&Action::Session(SessionAction::Save), state, audio, io_tx);
            pending_audio_dirty.merge(r.audio_dirty);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "load" => {
            let r = dispatch::dispatch_action(&Action::Session(SessionAction::Load), state, audio, io_tx);
            pending_audio_dirty.merge(r.audio_dirty);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "master_mute" => {
            let r = dispatch::dispatch_action(
                &Action::Session(SessionAction::ToggleMasterMute), state, audio, io_tx);
            pending_audio_dirty.merge(r.audio_dirty);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "record_master" => {
            let r = dispatch::dispatch_action(&Action::Server(ui::ServerAction::RecordMaster), state, audio, io_tx);
            pending_audio_dirty.merge(r.audio_dirty);
            apply_dispatch_result(r, state, panes, app_frame);
        }
        "switch:instrument" => {
            switch_to_pane("instrument_edit", panes, state, app_frame, layer_stack);
        }
        "switch:instrument_list" => {
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
        "switch:eq" => {
            switch_to_pane("eq", panes, state, app_frame, layer_stack);
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
                    "eq" => "Parametric EQ",
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
            sync_instrument_edit(state, panes);
        }
        "select_next_instrument" => {
            state.instruments.select_next();
            sync_piano_roll_to_selection(state, panes);
            sync_instrument_edit(state, panes);
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
        "add_instrument" => {
            switch_to_pane("add", panes, state, app_frame, layer_stack);
        }
        "delete_instrument" => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                let id = instrument.id;
                let r = dispatch::dispatch_action(&Action::Instrument(ui::InstrumentAction::Delete(id)), state, audio, io_tx);
                pending_audio_dirty.merge(r.audio_dirty);
                apply_dispatch_result(r, state, panes, app_frame);
                // Re-sync edit pane after deletion
                sync_instrument_edit(state, panes);
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
pub(crate) fn apply_status_events(events: &[StatusEvent], panes: &mut PaneManager) {
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
pub(crate) fn apply_dispatch_result(
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
            NavIntent::OpenVstParams(instrument_id, target) => {
                if let Some(vp) = panes.get_pane_mut::<VstParamPane>("vst_params") {
                    vp.set_target(*instrument_id, *target);
                }
                panes.push_to("vst_params", state);
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
