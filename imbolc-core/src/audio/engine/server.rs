use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{AudioEngine, ServerStatus, GROUP_SOURCES, GROUP_PROCESSING, GROUP_OUTPUT, GROUP_RECORD};
use crate::audio::osc_client::{AudioMonitor, OscClient};
use regex::Regex;

impl AudioEngine {
    #[allow(dead_code)]
    pub fn start_server(&mut self) -> Result<(), String> {
        self.start_server_with_devices(None, None)
    }

    pub fn start_server_with_devices(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<(), String> {
        if self.scsynth_process.is_some() {
            return Err("Server already running".to_string());
        }

        self.server_status = ServerStatus::Starting;

        let scsynth_paths = [
            "scsynth",
            "/Applications/SuperCollider.app/Contents/Resources/scsynth",
            "/usr/local/bin/scsynth",
            "/usr/bin/scsynth",
        ];

        // Build args: base port + optional device flags
        let mut args: Vec<String> = vec!["-u".to_string(), "57110".to_string()];

        // Resolve "System Default" to actual device names so we always
        // pass -H to scsynth. Without -H, scsynth probes all devices
        // and can crash on incompatible ones (e.g. iPhone continuity mic).
        let (default_output, default_input) = crate::audio::devices::default_device_names();
        let resolved_input = input_device
            .map(|s| s.to_string())
            .or(default_input);
        let resolved_output = output_device
            .map(|s| s.to_string())
            .or(default_output);

        match (resolved_input.as_deref(), resolved_output.as_deref()) {
            (Some(inp), Some(out)) if inp != out => {
                args.push("-H".to_string());
                args.push(inp.to_string());
                args.push(out.to_string());
            }
            (Some(dev), None) | (None, Some(dev)) => {
                args.push("-H".to_string());
                args.push(dev.to_string());
            }
            (Some(dev), Some(_)) => {
                // Same device for both
                args.push("-H".to_string());
                args.push(dev.to_string());
            }
            (None, None) => {}
        }

        // Redirect scsynth output to a log file for crash diagnostics
        let log_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("imbolc")
            .join("scsynth.log");
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        let stdout_file = fs::File::create(&log_path).ok();
        let stderr_file = stdout_file.as_ref().and_then(|f| f.try_clone().ok());

        let mut child = None;
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        for path in &scsynth_paths {
            match Command::new(path)
                .args(&arg_refs)
                .stdout(stdout_file.as_ref()
                    .and_then(|f| f.try_clone().ok())
                    .map(Stdio::from)
                    .unwrap_or_else(Stdio::null))
                .stderr(stderr_file.as_ref()
                    .and_then(|f| f.try_clone().ok())
                    .map(Stdio::from)
                    .unwrap_or_else(Stdio::null))
                .spawn()
            {
                Ok(c) => {
                    child = Some(c);
                    break;
                }
                Err(_) => continue,
            }
        }

        match child {
            Some(mut c) => {
                self.server_status = ServerStatus::Running;
                thread::sleep(Duration::from_millis(500));

                // Verify scsynth didn't crash during startup
                match c.try_wait() {
                    Ok(Some(status)) => {
                        self.server_status = ServerStatus::Error;
                        Err(format!(
                            "scsynth crashed ({}) — see {}",
                            status, log_path.display()
                        ))
                    }
                    _ => {
                        self.scsynth_process = Some(c);
                        Ok(())
                    }
                }
            }
            None => {
                self.server_status = ServerStatus::Error;
                Err("Could not find scsynth. Install SuperCollider.".to_string())
            }
        }
    }

    /// Check if the scsynth child process has exited unexpectedly.
    /// Returns `Some(message)` if it died, `None` if healthy.
    pub fn check_server_health(&mut self) -> Option<String> {
        if let Some(ref mut child) = self.scsynth_process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.scsynth_process = None;
                    self.is_running = false;
                    self.client = None;
                    self.server_status = ServerStatus::Error;
                    self.groups_created = false;
                    Some(format!("scsynth exited ({})", status))
                }
                _ => None,
            }
        } else if self.is_running {
            // is_running but no process — stale state
            self.is_running = false;
            self.server_status = ServerStatus::Error;
            self.groups_created = false;
            Some("scsynth process lost".to_string())
        } else {
            None
        }
    }

    pub fn stop_server(&mut self) {
        self.stop_recording();
        self.disconnect();
        if let Some(mut child) = self.scsynth_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.server_status = ServerStatus::Stopped;
    }

    pub fn compile_synthdefs_async(&mut self, scd_path: &Path) -> Result<(), String> {
        if self.is_compiling {
            return Err("Compilation already in progress".to_string());
        }
        if !scd_path.exists() {
            return Err(format!("File not found: {}", scd_path.display()));
        }

        if Self::synthdefs_are_fresh(scd_path) {
            let (tx, rx) = mpsc::channel();
            self.compile_receiver = Some(rx);
            self.is_compiling = true;
            let _ = tx.send(Ok("Synthdefs up-to-date, skipped compilation".to_string()));
            return Ok(());
        }

        let path = scd_path.to_path_buf();
        let (tx, rx) = mpsc::channel();
        self.compile_receiver = Some(rx);
        self.is_compiling = true;

        thread::spawn(move || {
            let result = Self::run_sclang(&path);
            let _ = tx.send(result);
        });

        Ok(())
    }

    /// Check if all `.scsyndef` files in the same directory as `scd_path` are
    /// newer than `scd_path` itself. Returns `true` if compilation can be skipped.
    fn synthdefs_are_fresh(scd_path: &Path) -> bool {
        let dir = match scd_path.parent() {
            Some(d) => d,
            None => return false,
        };

        let scd_mtime = match fs::metadata(scd_path).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => return false,
        };

        let content = match fs::read_to_string(scd_path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let name_re = match Regex::new(r#"SynthDef\s*\(\s*[\\"]([\w]+)"#) {
            Ok(re) => re,
            Err(_) => return false,
        };

        let mut names: HashSet<String> = HashSet::new();
        for caps in name_re.captures_iter(&content) {
            if let Some(name) = caps.get(1).map(|m| m.as_str().to_string()) {
                names.insert(name);
            }
        }

        if names.is_empty() {
            return false;
        }

        for name in names {
            let path = dir.join(format!("{name}.scsyndef"));
            let def_mtime = match fs::metadata(&path).and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => return false,
            };
            if def_mtime <= scd_mtime {
                return false;
            }
        }

        true
    }

    pub fn poll_compile_result(&mut self) -> Option<Result<String, String>> {
        if let Some(ref rx) = self.compile_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.compile_receiver = None;
                    self.is_compiling = false;
                    Some(result)
                }
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.compile_receiver = None;
                    self.is_compiling = false;
                    Some(Err("Compilation thread terminated unexpectedly".to_string()))
                }
            }
        } else {
            None
        }
    }

    fn run_sclang(scd_path: &PathBuf) -> Result<String, String> {
        let sclang_paths = [
            "sclang",
            "/Applications/SuperCollider.app/Contents/MacOS/sclang",
            "/usr/local/bin/sclang",
            "/usr/bin/sclang",
        ];

        for path in &sclang_paths {
            match Command::new(path).arg(scd_path).output() {
                Ok(output) => {
                    if output.status.success() {
                        return Ok("Synthdefs compiled successfully".to_string());
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(format!("Compilation failed: {}", stderr));
                    }
                }
                Err(_) => continue,
            }
        }

        Err("Could not find sclang. Install SuperCollider.".to_string())
    }

    pub fn connect(&mut self, server_addr: &str) -> std::io::Result<()> {
        let client = OscClient::new(server_addr)?;
        client.send_message("/notify", vec![rosc::OscType::Int(1)])?;
        self.client = Some(Box::new(client));
        self.is_running = true;
        self.server_status = ServerStatus::Connected;
        Ok(())
    }

    pub fn connect_with_monitor(&mut self, server_addr: &str, monitor: AudioMonitor) -> std::io::Result<()> {
        let client = OscClient::new_with_monitor(server_addr, monitor)?;
        client.send_message("/notify", vec![rosc::OscType::Int(1)])?;
        self.client = Some(Box::new(client));
        self.is_running = true;
        self.server_status = ServerStatus::Connected;
        Ok(())
    }

    pub(super) fn restart_meter(&mut self) {
        if let Some(node_id) = self.meter_node_id.take() {
            if let Some(ref client) = self.client {
                let _ = client.free_node(node_id);
            }
        }
        // Free existing analysis synths
        if let Some(ref client) = self.client {
            for &node_id in &self.analysis_node_ids {
                let _ = client.free_node(node_id);
            }
        }
        self.analysis_node_ids.clear();

        if let Some(ref client) = self.client {
            // Create meter synth
            let node_id = self.next_node_id;
            self.next_node_id += 1;
            let args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("imbolc_meter".to_string()),
                rosc::OscType::Int(node_id),
                rosc::OscType::Int(3), // addAfter
                rosc::OscType::Int(GROUP_OUTPUT),
            ];
            if client.send_message("/s_new", args).is_ok() {
                self.meter_node_id = Some(node_id);
            }

            // Create analysis synths (spectrum, LUFS, scope)
            for synth_def in &["imbolc_spectrum", "imbolc_lufs_meter", "imbolc_scope"] {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                let args: Vec<rosc::OscType> = vec![
                    rosc::OscType::String(synth_def.to_string()),
                    rosc::OscType::Int(node_id),
                    rosc::OscType::Int(3), // addAfter
                    rosc::OscType::Int(GROUP_OUTPUT),
                ];
                if client.send_message("/s_new", args).is_ok() {
                    self.analysis_node_ids.push(node_id);
                }
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.stop_recording();
        if let Some(ref client) = self.client {
            if let Some(node_id) = self.meter_node_id.take() {
                let _ = client.free_node(node_id);
            }
            for &node_id in &self.analysis_node_ids {
                let _ = client.free_node(node_id);
            }
            for nodes in self.node_map.values() {
                for node_id in nodes.all_node_ids() {
                    let _ = client.free_node(node_id);
                }
            }
            // Free all loaded sample buffers
            for &bufnum in self.buffer_map.values() {
                let _ = client.free_buffer(bufnum);
            }
        }
        self.node_map.clear();
        self.send_node_map.clear();
        self.bus_node_map.clear();
        self.bus_audio_buses.clear();
        self.voice_chains.clear();
        self.analysis_node_ids.clear();
        self.buffer_map.clear();
        self.bus_allocator.reset();
        self.groups_created = false;
        self.wavetables_initialized = false;
        self.client = None;
        self.is_running = false;
        if self.scsynth_process.is_some() {
            self.server_status = ServerStatus::Running;
        } else {
            self.server_status = ServerStatus::Stopped;
        }
    }

    pub(super) fn ensure_groups(&mut self) -> Result<(), String> {
        if self.groups_created {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;
        client.create_group(GROUP_SOURCES, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_PROCESSING, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_OUTPUT, 1, 0).map_err(|e| e.to_string())?;
        client.create_group(GROUP_RECORD, 1, 0).map_err(|e| e.to_string())?;
        self.groups_created = true;
        Ok(())
    }
}
