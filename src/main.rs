#![allow(dead_code, unused_imports)]

mod audio;
mod core;
mod dispatch;
mod midi;
mod panes;
mod playback;
mod setup;
mod state;
mod ui;

use std::time::{Duration, Instant};

use audio::AudioEngine;
use panes::{AddPane, FrameEditPane, HelpPane, HomePane, MixerPane, PianoRollPane, SequencerPane, ServerPane, StripEditPane, StripPane};
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

    let mut audio_engine = AudioEngine::new();
    let mut app_frame = Frame::new();
    let mut last_frame_time = Instant::now();
    let mut active_notes: Vec<(u32, u8, u32)> = Vec::new();

    setup::auto_start_sc(&mut audio_engine, &mut panes, &mut app_frame);

    loop {
        if let Some(event) = backend.poll_event(Duration::from_millis(16)) {
            // Global Ctrl-Q to quit
            if event.key == KeyCode::Char('q') && event.modifiers.ctrl {
                break;
            }

            // Global Ctrl-S to save
            if event.key == KeyCode::Char('s') && event.modifiers.ctrl {
                dispatch::dispatch_action(&Action::SaveRack, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes);
                continue;
            }

            // Global Ctrl-L to load
            if event.key == KeyCode::Char('l') && event.modifiers.ctrl {
                dispatch::dispatch_action(&Action::LoadRack, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes);
                continue;
            }

            // Global '.' to toggle master mute (works even in piano mode)
            if event.key == KeyCode::Char('.') {
                if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                    strip_pane.state_mut().master_mute = !strip_pane.state().master_mute;
                }
                if audio_engine.is_running() {
                    if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                        let _ = audio_engine.rebuild_strip_routing(strip_pane.state());
                    }
                }
                continue;
            }

            // Global number-key navigation (skip when pane wants exclusive input)
            if !panes.active().wants_exclusive_input() {
            if let KeyCode::Char(c) = event.key {
                // Helper to capture current view state
                let capture_view = |panes: &mut PaneManager| -> ViewState {
                    let pane_id = panes.active().id().to_string();
                    let strip_selection = panes.get_pane_mut::<StripPane>("strip")
                        .map(|sp| sp.state().selected)
                        .unwrap_or(None);
                    let edit_tab = panes.get_pane_mut::<StripEditPane>("strip_edit")
                        .map(|ep| ep.tab_index())
                        .unwrap_or(0);
                    ViewState { pane_id, strip_selection, edit_tab }
                };

                // Helper to restore view state
                let restore_view = |panes: &mut PaneManager, view: &ViewState| {
                    // Restore strip selection first
                    if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                        strip_pane.state_mut().selected = view.strip_selection;
                    }
                    // Restore edit tab
                    if let Some(edit_pane) = panes.get_pane_mut::<StripEditPane>("strip_edit") {
                        edit_pane.set_tab_index(view.edit_tab);
                    }
                    // Switch to the pane
                    panes.switch_to(&view.pane_id);
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
                            let current = capture_view(&mut panes);
                            app_frame.forward_view = Some(current);
                            restore_view(&mut panes, &back);
                        }
                        continue;
                    }
                    '~' => {
                        // Forward navigation
                        if let Some(forward) = app_frame.forward_view.take() {
                            let current = capture_view(&mut panes);
                            app_frame.back_view = Some(current);
                            restore_view(&mut panes, &forward);
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
                    let current = capture_view(&mut panes);
                    app_frame.back_view = Some(current);
                    app_frame.forward_view = None;
                    panes.switch_to(id);
                    continue;
                }
            }
            }

            let action = panes.handle_input(event);
            if dispatch::dispatch_action(&action, &mut panes, &mut audio_engine, &mut app_frame, &mut active_notes) {
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
            playback::tick_playback(&mut panes, &mut audio_engine, &mut active_notes, elapsed);
        }

        // Update master meter from real audio peak
        {
            let peak = if audio_engine.is_running() {
                audio_engine.master_peak()
            } else {
                0.0
            };
            let mute = panes.get_pane_mut::<StripPane>("strip")
                .map(|sp| sp.state().master_mute)
                .unwrap_or(false);
            app_frame.set_master_peak(peak, mute);
        }

        // Render
        let mut frame = backend.begin_frame()?;
        app_frame.render(&mut frame);

        let active_id = panes.active().id();
        if active_id == "mixer" {
            let strip_state = panes
                .get_pane_mut::<StripPane>("strip")
                .map(|sp| sp.state().clone());
            if let Some(state) = strip_state {
                if let Some(mixer_pane) = panes.get_pane_mut::<MixerPane>("mixer") {
                    mixer_pane.render_with_state(&mut frame, &state);
                }
            }
        } else if active_id == "piano_roll" {
            // Get state and current track info for piano roll rendering
            let (strip_state, pr_state, current_track) = {
                let strip_pane = panes.get_pane_mut::<StripPane>("strip");
                let state = strip_pane.map(|sp| sp.state().clone());
                let pr = state.as_ref().map(|s| s.piano_roll.clone());
                let track_idx = panes
                    .get_pane_mut::<PianoRollPane>("piano_roll")
                    .map(|p| p.current_track())
                    .unwrap_or(0);
                (state, pr, track_idx)
            };

            if let (Some(ss), Some(pr)) = (strip_state, pr_state) {
                // Get waveform data if current track is an AudioIn strip
                let waveform: Option<Vec<f32>> = pr.track_at(current_track)
                    .and_then(|track| ss.strip(track.module_id))
                    .filter(|strip| strip.source.is_audio_input())
                    .map(|strip| audio_engine.audio_in_waveform(strip.id));

                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.render_with_full_state(
                        &mut frame,
                        &pr,
                        &ss,
                        waveform.as_deref(),
                    );
                }
            }
        } else {
            panes.render(&mut frame);
        }

        backend.end_frame(frame)?;
    }

    Ok(())
}
