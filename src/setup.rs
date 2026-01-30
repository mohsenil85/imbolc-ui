use crate::audio::devices;
use crate::audio::{self, AudioEngine};
use crate::panes::ServerPane;
use crate::state::AppState;
use crate::ui::PaneManager;

/// Auto-start SuperCollider server, connect, and load synthdefs.
pub fn auto_start_sc(
    audio_engine: &mut AudioEngine,
    state: &AppState,
    panes: &mut PaneManager,
) {
    // Load saved device preferences
    let config = devices::load_device_config();

    match audio_engine.start_server_with_devices(
        config.input_device.as_deref(),
        config.output_device.as_deref(),
    ) {
        Ok(()) => {
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Running, "Server started");
                server.set_server_running(true);
            }
            match audio_engine.connect("127.0.0.1:57110") {
                Ok(()) => {
                    let synthdef_dir = std::path::Path::new("synthdefs");
                    if let Err(e) = audio_engine.load_synthdefs(synthdef_dir) {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(
                                audio::ServerStatus::Connected,
                                &format!("Connected (synthdef warning: {})", e),
                            );
                        }
                    } else {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(audio::ServerStatus::Connected, "Connected + synthdefs loaded");
                        }
                        // Wait for scsynth to finish processing /d_recv messages
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Rebuild routing
                        let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                }
                Err(e) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Running, &format!("Server running (connect failed: {})", e));
                    }
                }
            }
        }
        Err(_e) => {
            // Server start failed — status remains Stopped
        }
    }
}
