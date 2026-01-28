#![allow(dead_code, unused_imports)]

mod audio;
mod core;
mod dispatch;
mod panes;
mod playback;
mod setup;
mod state;
mod ui;

use std::time::{Duration, Instant};

use audio::AudioEngine;
use panes::{AddPane, FrameEditPane, HelpPane, HomePane, MixerPane, PianoRollPane, SequencerPane, ServerPane, StripEditPane, StripPane};
use ui::{
    Action, Frame, InputSource, KeyCode, PaneManager, RatatuiBackend,
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

            // Global number-key navigation
            if let KeyCode::Char(c) = event.key {
                let target = match c {
                    '1' => Some("strip"),
                    '2' => Some("piano_roll"),
                    '3' => Some("sequencer"),
                    '4' => Some("mixer"),
                    '5' => Some("server"),
                    '`' => {
                        // Sync piano roll state into session before editing
                        if let Some(strip_pane) = panes.get_pane_mut::<StripPane>("strip") {
                            let pr = &strip_pane.state().piano_roll;
                            app_frame.session.time_signature = pr.time_signature;
                            app_frame.session.bpm = pr.bpm as u16;
                        }
                        let session = app_frame.session.clone();
                        if let Some(fe) = panes.get_pane_mut::<FrameEditPane>("frame_edit") {
                            fe.set_session(session);
                        }
                        Some("frame_edit")
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
                    panes.switch_to(id);
                    continue;
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
            let pr_state = panes
                .get_pane_mut::<StripPane>("strip")
                .map(|sp| sp.state().piano_roll.clone());
            if let Some(pr) = pr_state {
                if let Some(pr_pane) = panes.get_pane_mut::<PianoRollPane>("piano_roll") {
                    pr_pane.render_with_state(&mut frame, &pr);
                }
            }
        } else {
            panes.render(&mut frame);
        }

        backend.end_frame(frame)?;
    }

    Ok(())
}
