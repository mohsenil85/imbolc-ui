use std::path::PathBuf;

use crate::audio::{self, AudioEngine};
use crate::panes::ServerPane;
use crate::state::AppState;
use crate::ui::{PaneManager, ServerAction};

pub(super) fn dispatch_server(
    action: &ServerAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        ServerAction::Connect => {
            let result = audio_engine.connect("127.0.0.1:57110");
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => {
                        // Load built-in synthdefs
                        let synthdef_dir = std::path::Path::new("synthdefs");
                        let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

                        // Also load custom synthdefs from config dir
                        let config_dir = config_synthdefs_dir();
                        let custom_result = if config_dir.exists() {
                            audio_engine.load_synthdefs(&config_dir)
                        } else {
                            Ok(())
                        };

                        // Load drum sequencer samples for all drum machine instruments
                        for instrument in &state.instruments.instruments {
                            if let Some(seq) = &instrument.drum_sequencer {
                                for pad in &seq.pads {
                                    if let Some(buffer_id) = pad.buffer_id {
                                        if let Some(ref path) = pad.path {
                                            let _ = audio_engine.load_sample(buffer_id, path);
                                        }
                                    }
                                }
                            }
                        }

                        match (builtin_result, custom_result) {
                            (Ok(()), Ok(())) => {
                                server.set_status(audio::ServerStatus::Connected, "Connected");
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                server.set_status(
                                    audio::ServerStatus::Connected,
                                    &format!("Connected (synthdef warning: {})", e),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        server.set_status(audio::ServerStatus::Error, &e.to_string())
                    }
                }
            }
        }
        ServerAction::Disconnect => {
            audio_engine.disconnect();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio_engine.status(), "Disconnected");
                server.set_server_running(audio_engine.server_running());
            }
        }
        ServerAction::Start => {
            let (input_dev, output_dev) = panes.get_pane_mut::<ServerPane>("server")
                .map(|s| (s.selected_input_device(), s.selected_output_device()))
                .unwrap_or((None, None));
            let result = audio_engine.start_server_with_devices(
                input_dev.as_deref(),
                output_dev.as_deref(),
            );
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match result {
                    Ok(()) => {
                        server.set_status(audio::ServerStatus::Running, "Server started");
                        server.set_server_running(true);
                    }
                    Err(e) => {
                        server.set_status(audio::ServerStatus::Error, &e);
                        server.set_server_running(false);
                    }
                }
            }
        }
        ServerAction::Stop => {
            audio_engine.stop_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Stopped, "Server stopped");
                server.set_server_running(false);
            }
        }
        ServerAction::CompileSynthDefs => {
            let scd_path = std::path::Path::new("synthdefs/compile.scd");
            match audio_engine.compile_synthdefs_async(scd_path) {
                Ok(()) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio_engine.status(), "Compiling synthdefs...");
                    }
                }
                Err(e) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio_engine.status(), &e);
                    }
                }
            }
        }
        ServerAction::LoadSynthDefs => {
            // Load built-in synthdefs
            let synthdef_dir = std::path::Path::new("synthdefs");
            let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

            // Also load custom synthdefs from config dir
            let config_dir = config_synthdefs_dir();
            let custom_result = if config_dir.exists() {
                audio_engine.load_synthdefs(&config_dir)
            } else {
                Ok(())
            };

            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                match (builtin_result, custom_result) {
                    (Ok(()), Ok(())) => {
                        server.set_status(audio_engine.status(), "Synthdefs loaded (built-in + custom)");
                    }
                    (Err(e), _) => {
                        server.set_status(audio_engine.status(), &format!("Error loading built-in: {}", e));
                    }
                    (_, Err(e)) => {
                        server.set_status(audio_engine.status(), &format!("Error loading custom: {}", e));
                    }
                }
            }
        }
        ServerAction::RecordMaster => {
            if audio_engine.is_recording() {
                if let Some(path) = audio_engine.stop_recording() {
                    // Auto-deactivate AudioIn instrument on stop
                    if let Some(inst) = state.instruments.selected_instrument_mut() {
                        if inst.source.is_audio_input() && inst.active {
                            inst.active = false;
                            let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                        }
                    }
                    // Defer waveform load — scsynth needs time to flush the WAV
                    state.pending_recording_path = Some(path.clone());
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(
                            audio_engine.status(),
                            &format!("Recording saved: {}", path.display()),
                        );
                    }
                }
            } else if audio_engine.is_running() {
                // Auto-activate AudioIn instrument on start
                if let Some(inst) = state.instruments.selected_instrument_mut() {
                    if inst.source.is_audio_input() && !inst.active {
                        inst.active = true;
                        let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                }
                let path = super::recording_path("master");
                match audio_engine.start_recording(0, &path) {
                    Ok(()) => {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(
                                audio_engine.status(),
                                &format!("Recording to {}", path.display()),
                            );
                        }
                    }
                    Err(e) => {
                        if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                            server.set_status(audio_engine.status(), &format!("Record error: {}", e));
                        }
                    }
                }
            }
        }
        ServerAction::RecordInput => {
            if audio_engine.is_recording() {
                if let Some(path) = audio_engine.stop_recording() {
                    // Auto-deactivate AudioIn instrument on stop
                    if let Some(inst) = state.instruments.selected_instrument_mut() {
                        if inst.source.is_audio_input() && inst.active {
                            inst.active = false;
                            let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                        }
                    }
                    // Defer waveform load — scsynth needs time to flush the WAV
                    state.pending_recording_path = Some(path.clone());
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(
                            audio_engine.status(),
                            &format!("Recording saved: {}", path.display()),
                        );
                    }
                }
            } else if audio_engine.is_running() {
                // Record from the selected instrument's source_out bus
                if let Some(inst) = state.instruments.selected_instrument() {
                    let inst_id = inst.id;
                    // Auto-activate AudioIn instrument on start
                    if inst.source.is_audio_input() && !inst.active {
                        if let Some(inst_mut) = state.instruments.instrument_mut(inst_id) {
                            inst_mut.active = true;
                        }
                        let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);
                    }
                    let path = super::recording_path(&format!("input_{}", inst_id));
                    // Bus 0 is hardware out; for instrument recording we use bus 0
                    // since instruments route through output to bus 0
                    match audio_engine.start_recording(0, &path) {
                        Ok(()) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(
                                    audio_engine.status(),
                                    &format!("Recording to {}", path.display()),
                                );
                            }
                        }
                        Err(e) => {
                            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                                server.set_status(audio_engine.status(), &format!("Record error: {}", e));
                            }
                        }
                    }
                }
            }
        }
        ServerAction::Restart => {
            // Get selected devices before stopping
            let (input_dev, output_dev) = panes.get_pane_mut::<ServerPane>("server")
                .map(|s| (s.selected_input_device(), s.selected_output_device()))
                .unwrap_or((None, None));

            // Stop
            audio_engine.stop_server();
            if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                server.set_status(audio::ServerStatus::Stopped, "Restarting server...");
                server.set_server_running(false);
            }

            // Start with selected devices
            let start_result = audio_engine.start_server_with_devices(
                input_dev.as_deref(),
                output_dev.as_deref(),
            );
            match start_result {
                Ok(()) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Running, "Server restarted, connecting...");
                        server.set_server_running(true);
                    }

                    // Connect
                    let connect_result = audio_engine.connect("127.0.0.1:57110");
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        match connect_result {
                            Ok(()) => {
                                // Load built-in synthdefs
                                let synthdef_dir = std::path::Path::new("synthdefs");
                                let builtin_result = audio_engine.load_synthdefs(synthdef_dir);

                                // Load custom synthdefs
                                let config_dir = config_synthdefs_dir();
                                let custom_result = if config_dir.exists() {
                                    audio_engine.load_synthdefs(&config_dir)
                                } else {
                                    Ok(())
                                };

                                // Load drum samples
                                for instrument in &state.instruments.instruments {
                                    if let Some(seq) = &instrument.drum_sequencer {
                                        for pad in &seq.pads {
                                            if let Some(buffer_id) = pad.buffer_id {
                                                if let Some(ref path) = pad.path {
                                                    let _ = audio_engine.load_sample(buffer_id, path);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Rebuild instrument routing
                                let _ = audio_engine.rebuild_instrument_routing(&state.instruments, &state.session);

                                match (builtin_result, custom_result) {
                                    (Ok(()), Ok(())) => {
                                        server.set_status(audio::ServerStatus::Connected, "Server restarted");
                                    }
                                    (Err(e), _) | (_, Err(e)) => {
                                        server.set_status(
                                            audio::ServerStatus::Connected,
                                            &format!("Restarted (synthdef warning: {})", e),
                                        );
                                    }
                                }
                                server.clear_device_config_dirty();
                            }
                            Err(e) => {
                                server.set_status(audio::ServerStatus::Error, &e.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(server) = panes.get_pane_mut::<ServerPane>("server") {
                        server.set_status(audio::ServerStatus::Error, &e);
                        server.set_server_running(false);
                    }
                }
            }
        }
    }
}

/// Get the config directory for custom synthdefs
pub(super) fn config_synthdefs_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("ilex")
            .join("synthdefs")
    } else {
        PathBuf::from("synthdefs")
    }
}

/// Find sclang executable, checking common locations
pub(super) fn find_sclang() -> Option<PathBuf> {
    // Check if sclang is in PATH
    if let Ok(output) = std::process::Command::new("which").arg("sclang").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Common macOS locations
    let candidates = [
        "/Applications/SuperCollider.app/Contents/MacOS/sclang",
        "/Applications/SuperCollider/SuperCollider.app/Contents/MacOS/sclang",
        "/usr/local/bin/sclang",
        "/opt/homebrew/bin/sclang",
    ];

    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Compile a .scd file using sclang and load it into scsynth
pub(super) fn compile_and_load_synthdef(
    scd_path: &std::path::Path,
    output_dir: &std::path::Path,
    synthdef_name: &str,
    audio_engine: &mut AudioEngine,
) -> Result<(), String> {
    // Find sclang
    let sclang = find_sclang().ok_or_else(|| {
        "sclang not found. Install SuperCollider or add sclang to PATH.".to_string()
    })?;

    // Read the original .scd file
    let scd_content = std::fs::read_to_string(scd_path)
        .map_err(|e| format!("Failed to read .scd file: {}", e))?;

    // Replace directory references with the actual output directory
    // Handle both patterns: `dir ? thisProcess...` and just `thisProcess...`
    let output_dir_str = format!("\"{}\"", output_dir.display());
    let modified_content = scd_content
        .replace("dir ? thisProcess.nowExecutingPath.dirname", &output_dir_str)
        .replace("thisProcess.nowExecutingPath.dirname", &output_dir_str);

    // Wrap in a block that exits when done
    let compile_script = format!(
        "(\n{}\n\"SUCCESS\".postln;\n0.exit;\n)",
        modified_content
    );

    // Write temp compile script
    let temp_script = std::env::temp_dir().join("ilex_compile_custom.scd");
    std::fs::write(&temp_script, &compile_script)
        .map_err(|e| format!("Failed to write compile script: {}", e))?;

    // Run sclang with a timeout by spawning and waiting
    let mut child = std::process::Command::new(&sclang)
        .arg(&temp_script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run sclang: {}", e))?;

    // Wait up to 30 seconds for compilation
    let timeout = std::time::Duration::from_secs(30);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("sclang compilation timed out".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Error waiting for sclang: {}", e)),
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to get sclang output: {}", e))?;

    // Check for errors (but ignore common non-error messages)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Look for actual errors, not just any "ERROR" in output
    let has_error = stderr.lines().any(|line| {
        line.contains("ERROR:") || line.contains("FAILURE")
    }) || stdout.lines().any(|line| {
        line.starts_with("ERROR:") || line.contains("FAILURE")
    });

    if has_error {
        return Err(format!("sclang error: {}{}", stdout, stderr));
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_script);

    // Load the .scsyndef into scsynth if connected
    if audio_engine.is_running() {
        let scsyndef_path = output_dir.join(format!("{}.scsyndef", synthdef_name));
        if scsyndef_path.exists() {
            audio_engine.load_synthdef_file(&scsyndef_path)?;
        } else {
            // Try loading all synthdefs from the directory as fallback
            audio_engine.load_synthdefs(output_dir)?;
        }
    }

    Ok(())
}
