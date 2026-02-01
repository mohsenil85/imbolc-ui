mod automation;
mod helpers;
mod instrument;
mod mixer;
mod piano_roll;
mod sequencer;
mod server;
mod session;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audio::AudioEngine;
use crate::state::AppState;
use crate::ui::{Action, Frame, PaneManager};

pub use helpers::compute_waveform_peaks;

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
        Action::Instrument(a) => instrument::dispatch_instrument(a, state, panes, audio_engine, active_notes),
        Action::Mixer(a) => mixer::dispatch_mixer(a, state, audio_engine),
        Action::PianoRoll(a) => piano_roll::dispatch_piano_roll(a, state, panes, audio_engine, active_notes),
        Action::Server(a) => server::dispatch_server(a, state, panes, audio_engine),
        Action::Session(a) => session::dispatch_session(a, state, panes, audio_engine, app_frame),
        Action::Sequencer(a) => sequencer::dispatch_sequencer(a, state, panes, audio_engine),
        Action::Chopper(a) => sequencer::dispatch_chopper(a, state, panes, audio_engine),
        Action::Automation(a) => automation::dispatch_automation(a, state, audio_engine),
        Action::None => {}
        // Layer management actions — handled in main.rs before dispatch
        Action::ExitPerformanceMode | Action::PushLayer(_) | Action::PopLayer(_) => {}
    }
    false
}
