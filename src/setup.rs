use crate::audio::devices;
use crate::audio::{self, AudioHandle};
use crate::state::AppState;
use crate::ui::StatusEvent;

/// Auto-start SuperCollider server, connect, and load synthdefs.
/// Returns status events for the UI layer to forward to the server pane.
pub fn auto_start_sc(
    audio: &mut AudioHandle,
    state: &AppState,
) -> Vec<StatusEvent> {
    let mut events = Vec::new();

    // Load saved device preferences
    let config = devices::load_device_config();

    match audio.start_server_with_devices(
        config.input_device.as_deref(),
        config.output_device.as_deref(),
    ) {
        Ok(()) => {
            events.push(StatusEvent {
                status: audio::ServerStatus::Running,
                message: "Server started".to_string(),
                server_running: Some(true),
            });
            match audio.connect("127.0.0.1:57110") {
                Ok(()) => {
                    let synthdef_dir = std::path::Path::new("synthdefs");
                    if let Err(e) = audio.load_synthdefs(synthdef_dir) {
                        events.push(StatusEvent {
                            status: audio::ServerStatus::Connected,
                            message: format!("Connected (synthdef warning: {})", e),
                            server_running: None,
                        });
                    } else {
                        events.push(StatusEvent {
                            status: audio::ServerStatus::Connected,
                            message: "Connected + synthdefs loaded".to_string(),
                            server_running: None,
                        });
                        // Wait for scsynth to finish processing /d_recv messages
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Rebuild routing
                        let _ = audio.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                }
                Err(e) => {
                    events.push(StatusEvent {
                        status: audio::ServerStatus::Running,
                        message: format!("Server running (connect failed: {})", e),
                        server_running: None,
                    });
                }
            }
        }
        Err(_e) => {
            // Server start failed — status remains Stopped
        }
    }

    events
}
