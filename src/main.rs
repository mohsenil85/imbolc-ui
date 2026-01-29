#![allow(dead_code, unused_imports)]

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
use panes::{AddPane, FileBrowserPane, FrameEditPane, HelpPane, HomePane, MixerPane, PianoRollPane, SequencerPane, ServerPane, StripEditPane, StripPane};
use state::AppState;
use ui::{
    Action, Frame, InputSource, KeyCode, PaneManager, RatatuiBackend, ViewState,
};

fn main() -> std::io::Result<()> {
    let mut backend = RatatuiBackend::new()?;
    backend.start()?;

    let result = run(&mut backend);

    backend.stop()?;
    result
}

fn run(backend: &mut RatatuiBackend) -> std::io::Result<()> {
    let mut state = AppState::new();
    let mut panes = PaneManager::new(Box::new(StripPane::new()));
    panes.add_pane(Box::new(HomePane::new()));
    panes.add_pane(Box::new(AddPane::new()));
    panes.add_pane(Box::new(StripEditPane::new()));
    panes.add_pane(Box::new(ServerPane::new()));
    panes.add_pane(Box::new(MixerPane::new()));
    panes.add_pane(Box::new(HelpPane::new()));
    panes.add_pane(Box::new(PianoRollPane::new()));
    panes.add_pane(Box::new(SequencerPane::new()));
    panes.add_pane(Box::new(FrameEditPane::new()));
    panes.add_pane(Box::new(FileBrowserPane::new()));

    let mut audio_engine = AudioEngine::new();
    let mut app_frame = Frame::new();
    let mut last_frame_time = Instant::now();
    let mut active_notes: Vec<(u32, u8, u32)> = Vec::new();

    setup::auto_start_sc(&mut audio_engine, &state, &mut panes, &mut app_frame);

    loop {
        if let Some(event) = backend.poll_event(Duration::from_millis(16)) {
            // Global Ctrl-Q to quit
            if event.key == KeyCode::Char('q') && event.modifiers.ctrl {
                break;
            }

            // Global Ctrl-S to save
            if event.key == KeyCode::Char('s') && event.modifiers.ctrl {
                dispatch::dispatch_action(&Action::SaveRack, &mut state, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes);
                continue;
            }

            // Global Ctrl-L to load
            if event.key == KeyCode::Char('l') && event.modifiers.ctrl {
                dispatch::dispatch_action(&Action::LoadRack, &mut state, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes);
                continue;
            }

            // Global '.' to toggle master mute (works even in piano mode)
            if event.key == KeyCode::Char('.') {
                state.strip.master_mute = !state.strip.master_mute;
                if audio_engine.is_running() {
                    let _ = audio_engine.update_all_strip_mixer_params(&state.strip);
                }
                continue;
            }

            // Global number-key navigation (skip when pane wants exclusive input)
            if !panes.active().wants_exclusive_input() {
            if let KeyCode::Char(c) = event.key {
                // Helper to capture current view state
                let capture_view = |panes: &mut PaneManager, state: &AppState| -> ViewState {
                    let pane_id = panes.active().id().to_string();
                    let strip_selection = state.strip.selected;
                    let edit_tab = panes.get_pane_mut::<StripEditPane>("strip_edit")
                        .map(|ep| ep.tab_index())
                        .unwrap_or(0);
                    ViewState { pane_id, strip_selection, edit_tab }
                };

                // Helper to restore view state
                let restore_view = |panes: &mut PaneManager, state: &mut AppState, view: &ViewState| {
                    // Restore strip selection first
                    state.strip.selected = view.strip_selection;
                    // Restore edit tab
                    if let Some(edit_pane) = panes.get_pane_mut::<StripEditPane>("strip_edit") {
                        edit_pane.set_tab_index(view.edit_tab);
                    }
                    // Switch to the pane
                    panes.switch_to(&view.pane_id, &*state);
                };

                let target = match c {
                    '1' => Some("strip"),
                    '2' => Some("piano_roll"),
                    '3' => Some("sequencer"),
                    '4' => Some("mixer"),
                    '5' => Some("server"),
                    '`' => {
                        // Back navigation
                        if let Some(back) = app_frame.back_view.take() {
                            let current = capture_view(&mut panes, &state);
                            app_frame.forward_view = Some(current);
                            restore_view(&mut panes, &mut state, &back);
                        }
                        continue;
                    }
                    '~' => {
                        // Forward navigation
                        if let Some(forward) = app_frame.forward_view.take() {
                            let current = capture_view(&mut panes, &state);
                            app_frame.back_view = Some(current);
                            restore_view(&mut panes, &mut state, &forward);
                        }
                        continue;
                    }
                    '?' => {
                        if panes.active().id() != "help" {
                            let current_id = panes.active().id();
                            let current_keymap = panes.active().keymap().clone();
                            let title = match current_id {
                                "strip" => "Strips",
                                "mixer" => "Mixer",
                                "server" => "Server",
                                "piano_roll" => "Piano Roll",
                                "sequencer" => "Sequencer",
                                "add" => "Add Strip",
                                "strip_edit" => "Edit Strip",
                                _ => current_id,
                            };
                            if let Some(help) = panes.get_pane_mut::<HelpPane>("help") {
                                help.set_context(current_id, title, &current_keymap);
                            }
                            Some("help")
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(id) = target {
                    // Save current view before switching
                    let current = capture_view(&mut panes, &state);
                    app_frame.back_view = Some(current);
                    app_frame.forward_view = None;
                    panes.switch_to(id, &state);
                    continue;
                }
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
        }

        // Update master meter from real audio peak
        {
            let peak = if audio_engine.is_running() {
                audio_engine.master_peak()
            } else {
                0.0
            };
            let mute = state.strip.master_mute;
            app_frame.set_master_peak(peak, mute);
        }

        // Update waveform cache for piano roll
        if panes.active().id() == "piano_roll" {
            let track = panes.get_pane_mut::<PianoRollPane>("piano_roll")
                .map(|p| p.current_track()).unwrap_or(0);
            state.audio_in_waveform = state.strip.piano_roll
                .track_at(track)
                .and_then(|t| state.strip.strip(t.module_id))
                .filter(|s| s.source.is_audio_input())
                .map(|s| audio_engine.audio_in_waveform(s.id));
        } else {
            state.audio_in_waveform = None;
        }

        // Render
        let mut frame = backend.begin_frame()?;
        app_frame.render(&mut frame);
        panes.render(&mut frame, &state);
        backend.end_frame(frame)?;
    }

    Ok(())
}
