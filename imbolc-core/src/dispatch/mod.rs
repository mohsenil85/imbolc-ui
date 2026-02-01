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

use crate::audio::AudioHandle;
use crate::state::AppState;
use crate::action::{Action, DispatchResult};

pub use helpers::compute_waveform_peaks;

/// Default path for save file
pub fn default_rack_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("imbolc")
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

/// Dispatch an action. Returns a DispatchResult describing side effects for the UI layer.
/// Dispatch no longer takes panes or app_frame — it operates purely on state and audio engine.
pub fn dispatch_action(
    action: &Action,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    let result = match action {
        Action::Quit => DispatchResult::with_quit(),
        Action::Nav(_) => DispatchResult::none(), // Handled by PaneManager
        Action::Instrument(a) => instrument::dispatch_instrument(a, state, audio),
        Action::Mixer(a) => mixer::dispatch_mixer(a, state, audio),
        Action::PianoRoll(a) => piano_roll::dispatch_piano_roll(a, state, audio),
        Action::Server(a) => server::dispatch_server(a, state, audio),
        Action::Session(a) => session::dispatch_session(a, state, audio),
        Action::Sequencer(a) => sequencer::dispatch_sequencer(a, state, audio),
        Action::Chopper(a) => sequencer::dispatch_chopper(a, state, audio),
        Action::Automation(a) => automation::dispatch_automation(a, state, audio),
        Action::None => DispatchResult::none(),
        // Layer management actions — handled in main.rs before dispatch
        Action::ExitPerformanceMode | Action::PushLayer(_) | Action::PopLayer(_) => DispatchResult::none(),
    };

    result
}
