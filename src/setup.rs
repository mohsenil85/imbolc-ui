use crate::audio::{self, AudioEngine};
use crate::panes::{RackPane, ServerPane};
use crate::ui::{Frame, PaneManager};

/// Auto-start SuperCollider server, connect, and load synthdefs.
pub fn auto_start_sc(
    audio_engine: &mut AudioEngine,
    panes: &mut PaneManager,
    app_frame: &mut Frame,
) {
    app_frame.push_message("SC: starting server...".to_string());
    match audio_engine.start_server() {
        Ok(()) => {
            app_frame.push_message("SC: server started on port 57110".to_string());
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Running, "Server started");
                server.set_server_running(true);
            }
            match audio_engine.connect("127.0.0.1:57110") {
                Ok(()) => {
                    app_frame.push_message("SC: connected".to_string());
                    let synthdef_dir = std::path::Path::new("synthdefs");
                    if let Err(e) = audio_engine.load_synthdefs(synthdef_dir) {
                        app_frame.push_message(format!("SC: synthdef warning: {}", e));
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(
                                audio::ServerStatus::Connected,
                                &format!("Connected (synthdef warning: {})", e),
                            );
                        }
                    } else {
                        app_frame.push_message("SC: synthdefs loaded".to_string());
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(audio::ServerStatus::Connected, "Connected + synthdefs loaded");
                        }
                        // Wait for scsynth to finish processing /d_recv messages
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        // Rebuild routing to create groups and meter synth
                        if let Some(rack_pane) = panes.get_pane_mut::<RackPane>("rack") {
                            let _ = audio_engine.rebuild_routing(rack_pane.rack());
                        }
                    }
                }
                Err(e) => {
                    app_frame.push_message(format!("SC: connect failed: {}", e));
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Running, "Server running (connect failed)");
                    }
                }
            }
        }
        Err(e) => {
            app_frame.push_message(format!("SC: start failed: {}", e));
        }
    }
}
