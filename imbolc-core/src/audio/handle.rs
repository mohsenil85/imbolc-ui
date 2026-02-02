//! AudioHandle: main-thread interface to the audio engine.
//!
//! Owns the command/feedback channels and shared monitor state. The
//! AudioEngine and playback ticking live on the audio thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::commands::{AudioCmd, AudioFeedback};
use super::engine::AudioEngine;
use super::osc_client::AudioMonitor;
use super::snapshot::{AutomationSnapshot, InstrumentSnapshot, PianoRollSnapshot, SessionSnapshot};
use super::ServerStatus;
use crate::action::{AudioDirty, VstTarget};
use crate::state::automation::AutomationTarget;
use crate::state::piano_roll::PianoRollState;
use crate::state::{AppState, BufferId, InstrumentId, InstrumentState, SessionState};

/// Main-thread handle to the audio subsystem.
///
/// Phase 3: communicates with a dedicated audio thread via MPSC channels.
pub struct AudioHandle {
    cmd_tx: Sender<AudioCmd>,
    feedback_rx: Receiver<AudioFeedback>,
    monitor: AudioMonitor,
    status: ServerStatus,
    server_running: bool,
    is_running: bool,
    is_recording: bool,
    recording_elapsed: Option<Duration>,
    playhead: u32,
    bpm: f32,
    join_handle: Option<JoinHandle<()>>,
}

fn config_synthdefs_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("imbolc")
            .join("synthdefs")
    } else {
        PathBuf::from("synthdefs")
    }
}

impl AudioHandle {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (feedback_tx, feedback_rx) = mpsc::channel();
        let monitor = AudioMonitor::new();
        let thread_monitor = monitor.clone();

        let join_handle = thread::spawn(move || {
            let thread = AudioThread::new(cmd_rx, feedback_tx, thread_monitor);
            thread.run();
        });

        Self {
            cmd_tx,
            feedback_rx,
            monitor,
            status: ServerStatus::Stopped,
            server_running: false,
            is_running: false,
            is_recording: false,
            recording_elapsed: None,
            playhead: 0,
            bpm: 120.0,
            join_handle: Some(join_handle),
        }
    }

    pub fn send_cmd(&self, cmd: AudioCmd) -> Result<(), String> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| "Audio thread disconnected".to_string())
    }

    pub fn drain_feedback(&mut self) -> Vec<AudioFeedback> {
        let mut out = Vec::new();
        while let Ok(msg) = self.feedback_rx.try_recv() {
            self.apply_feedback(&msg);
            out.push(msg);
        }
        out
    }

    fn apply_feedback(&mut self, feedback: &AudioFeedback) {
        match feedback {
            AudioFeedback::PlayheadPosition(pos) => {
                self.playhead = *pos;
            }
            AudioFeedback::BpmUpdate(bpm) => {
                self.bpm = *bpm;
            }
            AudioFeedback::DrumSequencerStep { .. } => {}
            AudioFeedback::ServerStatus { status, server_running, .. } => {
                self.status = *status;
                self.server_running = *server_running;
                self.is_running = matches!(status, ServerStatus::Connected);
            }
            AudioFeedback::RecordingState { is_recording, elapsed_secs } => {
                self.is_recording = *is_recording;
                self.recording_elapsed = if *is_recording {
                    Some(Duration::from_secs(*elapsed_secs))
                } else {
                    None
                };
            }
            AudioFeedback::RecordingStopped(_) => {}
            AudioFeedback::CompileResult(_) => {}
            AudioFeedback::PendingBufferFreed => {}
            AudioFeedback::VstParamsDiscovered { .. } => {}
            AudioFeedback::VstStateSaved { .. } => {}
        }
    }

    pub fn sync_state(&mut self, state: &AppState) {
        self.flush_dirty(state, AudioDirty::all());
    }

    pub fn flush_dirty(&mut self, state: &AppState, dirty: AudioDirty) {
        if !dirty.any() {
            return;
        }

        let needs_state = dirty.instruments || dirty.session || dirty.routing || dirty.mixer_params;
        if needs_state {
            self.update_state(&state.instruments, &state.session);
        }
        if dirty.piano_roll {
            self.update_piano_roll_data(&state.session.piano_roll);
        }
        if dirty.automation {
            self.update_automation_lanes(&state.session.automation.lanes);
        }
        if dirty.routing {
            let _ = self.send_cmd(AudioCmd::RebuildRouting);
        }
        if dirty.mixer_params {
            let _ = self.send_cmd(AudioCmd::UpdateMixerParams);
        }
    }

    pub fn update_state(&mut self, instruments: &InstrumentSnapshot, session: &SessionSnapshot) {
        let _ = self.send_cmd(AudioCmd::UpdateState {
            instruments: instruments.clone(),
            session: session.clone(),
        });
    }

    pub fn update_piano_roll_data(&mut self, piano_roll: &PianoRollSnapshot) {
        let _ = self.send_cmd(AudioCmd::UpdatePianoRollData {
            piano_roll: piano_roll.clone(),
        });
    }

    pub fn update_automation_lanes(&mut self, lanes: &AutomationSnapshot) {
        let _ = self.send_cmd(AudioCmd::UpdateAutomationLanes {
            lanes: lanes.clone(),
        });
    }

    pub fn set_playing(&mut self, playing: bool) {
        let _ = self.send_cmd(AudioCmd::SetPlaying { playing });
    }

    pub fn reset_playhead(&mut self) {
        let _ = self.send_cmd(AudioCmd::ResetPlayhead);
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        let _ = self.send_cmd(AudioCmd::SetBpm { bpm });
    }

    // ── State accessors ───────────────────────────────────────────

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn status(&self) -> ServerStatus {
        self.status
    }

    pub fn server_running(&self) -> bool {
        self.server_running
    }

    pub fn master_peak(&self) -> f32 {
        let (l, r) = self.monitor.meter_peak();
        l.max(r)
    }

    pub fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.monitor.audio_in_waveform(instrument_id)
    }

    pub fn spectrum_bands(&self) -> [f32; 7] {
        self.monitor.spectrum_bands()
    }

    pub fn lufs_data(&self) -> (f32, f32, f32, f32) {
        self.monitor.lufs_data()
    }

    pub fn scope_buffer(&self) -> Vec<f32> {
        self.monitor.scope_buffer()
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    pub fn recording_elapsed(&self) -> Option<Duration> {
        self.recording_elapsed
    }

    // ── Server lifecycle ──────────────────────────────────────────

    pub fn connect_async(&mut self, server_addr: &str) -> Result<(), String> {
        let (reply_tx, _reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::Connect {
            server_addr: server_addr.to_string(),
            reply: reply_tx,
        })
    }

    pub fn disconnect_async(&mut self) -> Result<(), String> {
        self.send_cmd(AudioCmd::Disconnect)
    }

    pub fn start_server_async(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<(), String> {
        let (reply_tx, _reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::StartServer {
            input_device: input_device.map(|s| s.to_string()),
            output_device: output_device.map(|s| s.to_string()),
            reply: reply_tx,
        })
    }

    pub fn stop_server_async(&mut self) -> Result<(), String> {
        self.send_cmd(AudioCmd::StopServer)
    }

    pub fn restart_server_async(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
        server_addr: &str,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::RestartServer {
            input_device: input_device.map(|s| s.to_string()),
            output_device: output_device.map(|s| s.to_string()),
            server_addr: server_addr.to_string(),
        })
    }

    pub fn connect(&mut self, server_addr: &str) -> std::io::Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.cmd_tx
            .send(AudioCmd::Connect {
                server_addr: server_addr.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Audio thread disconnected"))?;
        match reply_rx.recv() {
            Ok(result) => {
                if result.is_ok() {
                    self.status = ServerStatus::Connected;
                    self.is_running = true;
                } else {
                    self.status = ServerStatus::Error;
                    self.is_running = false;
                }
                result
            }
            Err(_) => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Audio thread disconnected")),
        }
    }

    pub fn disconnect(&mut self) {
        let _ = self.send_cmd(AudioCmd::Disconnect);
        self.is_running = false;
        self.status = if self.server_running {
            ServerStatus::Running
        } else {
            ServerStatus::Stopped
        };
    }

    pub fn start_server_with_devices(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::StartServer {
            input_device: input_device.map(|s| s.to_string()),
            output_device: output_device.map(|s| s.to_string()),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => {
                if result.is_ok() {
                    self.status = ServerStatus::Running;
                    self.server_running = true;
                } else {
                    self.status = ServerStatus::Error;
                }
                result
            }
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    pub fn stop_server(&mut self) {
        let _ = self.send_cmd(AudioCmd::StopServer);
        self.status = ServerStatus::Stopped;
        self.server_running = false;
        self.is_running = false;
    }

    pub fn compile_synthdefs_async(&mut self, scd_path: &Path) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::CompileSynthDefs {
            scd_path: scd_path.to_path_buf(),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => result,
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    pub fn load_synthdefs(&self, dir: &Path) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::LoadSynthDefs {
            dir: dir.to_path_buf(),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => result,
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    pub fn load_synthdef_file(&self, path: &Path) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::LoadSynthDefFile {
            path: path.to_path_buf(),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => result,
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    // ── SynthDefs & samples ───────────────────────────────────────

    pub fn load_sample(&mut self, buffer_id: BufferId, path: &str) -> Result<i32, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::LoadSample {
            buffer_id,
            path: path.to_string(),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => result,
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    // ── Routing & mixing ──────────────────────────────────────────

    pub fn rebuild_instrument_routing(
        &mut self,
        _instruments: &InstrumentState,
        _session: &SessionState,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::RebuildRouting)
    }

    pub fn set_bus_mixer_params(
        &self,
        bus_id: u8,
        level: f32,
        mute: bool,
        pan: f32,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::SetBusMixerParams {
            bus_id,
            level,
            mute,
            pan,
        })
    }

    pub fn update_all_instrument_mixer_params(
        &self,
        _instruments: &InstrumentState,
        _session: &SessionState,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::UpdateMixerParams)
    }

    pub fn set_source_param(
        &self,
        instrument_id: InstrumentId,
        param: &str,
        value: f32,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::SetSourceParam {
            instrument_id,
            param: param.to_string(),
            value,
        })
    }

    pub fn set_eq_param(
        &self,
        instrument_id: InstrumentId,
        param: &str,
        value: f32,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::SetEqParam {
            instrument_id,
            param: param.to_string(),
            value,
        })
    }

    // ── Voice management ──────────────────────────────────────────

    pub fn spawn_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        _instruments: &InstrumentState,
        _session: &SessionState,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::SpawnVoice {
            instrument_id,
            pitch,
            velocity,
            offset_secs,
        })
    }

    pub fn release_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        offset_secs: f64,
        _instruments: &InstrumentState,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::ReleaseVoice {
            instrument_id,
            pitch,
            offset_secs,
        })
    }

    pub fn push_active_note(&mut self, instrument_id: u32, pitch: u8, duration_ticks: u32) {
        let _ = self.send_cmd(AudioCmd::RegisterActiveNote {
            instrument_id,
            pitch,
            duration_ticks,
        });
    }

    pub fn clear_active_notes(&mut self) {
        let _ = self.send_cmd(AudioCmd::ClearActiveNotes);
    }

    pub fn release_all_voices(&mut self) {
        let _ = self.send_cmd(AudioCmd::ReleaseAllVoices);
    }

    pub fn play_drum_hit_to_instrument(
        &mut self,
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::PlayDrumHit {
            buffer_id,
            amp,
            instrument_id,
            slice_start,
            slice_end,
        })
    }

    // ── Recording ─────────────────────────────────────────────────

    pub fn start_recording(&mut self, bus: i32, path: &Path) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send_cmd(AudioCmd::StartRecording {
            bus,
            path: path.to_path_buf(),
            reply: reply_tx,
        })?;
        match reply_rx.recv() {
            Ok(result) => {
                if result.is_ok() {
                    self.is_recording = true;
                    self.recording_elapsed = Some(Duration::from_secs(0));
                }
                result
            }
            Err(_) => Err("Audio thread disconnected".to_string()),
        }
    }

    pub fn stop_recording(&mut self) -> Option<std::path::PathBuf> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .send_cmd(AudioCmd::StopRecording { reply: reply_tx })
            .is_err()
        {
            return None;
        }
        match reply_rx.recv() {
            Ok(result) => {
                self.is_recording = false;
                self.recording_elapsed = None;
                result
            }
            Err(_) => None,
        }
    }

    // ── Automation ────────────────────────────────────────────────

    pub fn apply_automation(
        &self,
        target: &AutomationTarget,
        value: f32,
        _instruments: &InstrumentState,
        _session: &SessionState,
    ) -> Result<(), String> {
        self.send_cmd(AudioCmd::ApplyAutomation {
            target: target.clone(),
            value,
        })
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        let _ = self.send_cmd(AudioCmd::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Default for AudioHandle {
    fn default() -> Self {
        Self::new()
    }
}

struct AudioThread {
    engine: AudioEngine,
    cmd_rx: Receiver<AudioCmd>,
    feedback_tx: Sender<AudioFeedback>,
    monitor: AudioMonitor,
    instruments: InstrumentSnapshot,
    session: SessionSnapshot,
    piano_roll: PianoRollSnapshot,
    automation_lanes: AutomationSnapshot,
    active_notes: Vec<(u32, u8, u32)>,
    last_tick: Instant,
    last_recording_secs: u64,
    last_recording_state: bool,
    /// Simple LCG random seed for probability/humanization
    rng_state: u64,
    /// Per-instrument arpeggiator runtime state
    arp_states: std::collections::HashMap<u32, crate::state::arpeggiator::ArpPlayState>,
}

impl AudioThread {
    fn new(cmd_rx: Receiver<AudioCmd>, feedback_tx: Sender<AudioFeedback>, monitor: AudioMonitor) -> Self {
        Self {
            engine: AudioEngine::new(),
            cmd_rx,
            feedback_tx,
            monitor,
            instruments: InstrumentState::new(),
            session: SessionState::new(),
            piano_roll: PianoRollState::new(),
            automation_lanes: Vec::new(),
            active_notes: Vec::new(),
            last_tick: Instant::now(),
            last_recording_secs: 0,
            last_recording_state: false,
            rng_state: 12345,
            arp_states: std::collections::HashMap::new(),
        }
    }

    fn run(mut self) {
        loop {
            if self.drain_commands() {
                break;
            }

            let now = Instant::now();
            let elapsed = now.duration_since(self.last_tick);
            if elapsed >= Duration::from_millis(1) {
                self.last_tick = now;
                self.tick(elapsed);
            }

            self.poll_engine();
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn drain_commands(&mut self) -> bool {
        loop {
            match self.cmd_rx.try_recv() {
                Ok(cmd) => {
                    if self.handle_cmd(cmd) {
                        return true;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => return true,
            }
        }
    }

    fn handle_cmd(&mut self, cmd: AudioCmd) -> bool {
        match cmd {
            AudioCmd::Connect { server_addr, reply } => {
                let result = self.engine.connect_with_monitor(&server_addr, self.monitor.clone());
                match &result {
                    Ok(()) => {
                        let message = match self.load_synthdefs_and_samples() {
                            Ok(()) => "Connected".to_string(),
                            Err(e) => format!("Connected (synthdef warning: {})", e),
                        };
                        self.send_server_status(ServerStatus::Connected, message);
                    }
                    Err(err) => {
                        self.send_server_status(ServerStatus::Error, err.to_string());
                    }
                }
                let _ = reply.send(result);
            }
            AudioCmd::Disconnect => {
                self.engine.disconnect();
                self.send_server_status(self.engine.status(), "Disconnected");
            }
            AudioCmd::StartServer { input_device, output_device, reply } => {
                let result = self.engine.start_server_with_devices(
                    input_device.as_deref(),
                    output_device.as_deref(),
                );
                match &result {
                    Ok(()) => self.send_server_status(ServerStatus::Running, "Server started"),
                    Err(err) => self.send_server_status(ServerStatus::Error, err),
                }
                let _ = reply.send(result);
            }
            AudioCmd::StopServer => {
                self.engine.stop_server();
                self.send_server_status(ServerStatus::Stopped, "Server stopped");
            }
            AudioCmd::RestartServer { input_device, output_device, server_addr } => {
                self.engine.stop_server();
                self.send_server_status(ServerStatus::Stopped, "Restarting server...");

                let start_result = self.engine.start_server_with_devices(
                    input_device.as_deref(),
                    output_device.as_deref(),
                );
                match start_result {
                    Ok(()) => {
                        self.send_server_status(ServerStatus::Running, "Server restarted, connecting...");
                        let connect_result = self.engine.connect_with_monitor(&server_addr, self.monitor.clone());
                        match connect_result {
                            Ok(()) => {
                                let message = match self.load_synthdefs_and_samples() {
                                    Ok(()) => "Server restarted".to_string(),
                                    Err(e) => format!("Restarted (synthdef warning: {})", e),
                                };
                                self.send_server_status(ServerStatus::Connected, message);
                            }
                            Err(err) => {
                                self.send_server_status(ServerStatus::Error, err.to_string());
                            }
                        }
                    }
                    Err(err) => {
                        self.send_server_status(ServerStatus::Error, err);
                    }
                }
            }
            AudioCmd::CompileSynthDefs { scd_path, reply } => {
                let result = self.engine.compile_synthdefs_async(&scd_path);
                let _ = reply.send(result);
            }
            AudioCmd::LoadSynthDefs { dir, reply } => {
                let result = self.engine.load_synthdefs(&dir);
                let _ = reply.send(result);
            }
            AudioCmd::LoadSynthDefFile { path, reply } => {
                let result = self.engine.load_synthdef_file(&path);
                let _ = reply.send(result);
            }
            AudioCmd::UpdateState { instruments, session } => {
                self.apply_state_update(instruments, session);
            }
            AudioCmd::UpdatePianoRollData { piano_roll } => {
                self.apply_piano_roll_update(piano_roll);
            }
            AudioCmd::UpdateAutomationLanes { lanes } => {
                self.automation_lanes = lanes;
            }
            AudioCmd::SetPlaying { playing } => {
                self.piano_roll.playing = playing;
            }
            AudioCmd::ResetPlayhead => {
                self.piano_roll.playhead = 0;
                let _ = self.feedback_tx.send(AudioFeedback::PlayheadPosition(0));
            }
            AudioCmd::SetBpm { bpm } => {
                self.piano_roll.bpm = bpm;
                let _ = self.feedback_tx.send(AudioFeedback::BpmUpdate(bpm));
            }
            AudioCmd::RebuildRouting => {
                let _ = self.engine.rebuild_instrument_routing(&self.instruments, &self.session);
            }
            AudioCmd::UpdateMixerParams => {
                let _ = self.engine.update_all_instrument_mixer_params(&self.instruments, &self.session);
            }
            AudioCmd::SetBusMixerParams { bus_id, level, mute, pan } => {
                let _ = self.engine.set_bus_mixer_params(bus_id, level, mute, pan);
            }
            AudioCmd::SetSourceParam { instrument_id, param, value } => {
                let _ = self.engine.set_source_param(instrument_id, &param, value);
            }
            AudioCmd::SetEqParam { instrument_id, param, value } => {
                let _ = self.engine.set_eq_param(instrument_id, &param, value);
            }
            AudioCmd::SpawnVoice { instrument_id, pitch, velocity, offset_secs } => {
                let _ = self.engine.spawn_voice(instrument_id, pitch, velocity, offset_secs, &self.instruments, &self.session);
            }
            AudioCmd::ReleaseVoice { instrument_id, pitch, offset_secs } => {
                let _ = self.engine.release_voice(instrument_id, pitch, offset_secs, &self.instruments);
            }
            AudioCmd::RegisterActiveNote { instrument_id, pitch, duration_ticks } => {
                self.active_notes.push((instrument_id, pitch, duration_ticks));
            }
            AudioCmd::ClearActiveNotes => {
                self.active_notes.clear();
            }
            AudioCmd::ReleaseAllVoices => {
                self.engine.release_all_voices();
            }
            AudioCmd::PlayDrumHit { buffer_id, amp, instrument_id, slice_start, slice_end } => {
                let _ = self.engine.play_drum_hit_to_instrument(
                    buffer_id, amp, instrument_id, slice_start, slice_end,
                );
            }
            AudioCmd::LoadSample { buffer_id, path, reply } => {
                let result = self.engine.load_sample(buffer_id, &path);
                let _ = reply.send(result);
            }
            AudioCmd::StartRecording { bus, path, reply } => {
                let result = self.engine.start_recording(bus, &path);
                let _ = reply.send(result);
            }
            AudioCmd::StopRecording { reply } => {
                let path = self.engine.stop_recording();
                let _ = reply.send(path);
            }
            AudioCmd::ApplyAutomation { target, value } => {
                let _ = self.engine.apply_automation(&target, value, &self.instruments, &self.session);
            }
            AudioCmd::QueryVstParams { instrument_id, target } => {
                if let Some(node_id) = self.resolve_vst_node_id(instrument_id, target) {
                    let _ = self.engine.query_vst_param_count_node(node_id);
                }
                // Generate synthetic VstParamsDiscovered feedback (128 placeholder params)
                // since SC doesn't reply via OSC for param queries
                let vst_plugin_id = self.resolve_vst_plugin_id(instrument_id, target);
                if let Some(vst_plugin_id) = vst_plugin_id {
                    let params: Vec<(u32, String, Option<String>, f32)> = (0..128)
                        .map(|i| (i, format!("Param {}", i), None, 0.5))
                        .collect();
                    let _ = self.feedback_tx.send(AudioFeedback::VstParamsDiscovered {
                        instrument_id,
                        target,
                        vst_plugin_id,
                        params,
                    });
                }
            }
            AudioCmd::SetVstParam { instrument_id, target, param_index, value } => {
                if let Some(node_id) = self.resolve_vst_node_id(instrument_id, target) {
                    let _ = self.engine.set_vst_param_node(node_id, param_index, value);
                }
            }
            AudioCmd::SaveVstState { instrument_id, target, path } => {
                if let Some(node_id) = self.resolve_vst_node_id(instrument_id, target) {
                    let _ = self.engine.save_vst_state_node(node_id, &path);
                }
                let _ = self.feedback_tx.send(AudioFeedback::VstStateSaved {
                    instrument_id,
                    target,
                    path,
                });
            }
            AudioCmd::LoadVstState { instrument_id, target, path } => {
                if let Some(node_id) = self.resolve_vst_node_id(instrument_id, target) {
                    let _ = self.engine.load_vst_state_node(node_id, &path);
                }
            }
            AudioCmd::Shutdown => return true,
        }
        false
    }

    fn apply_state_update(&mut self, mut instruments: InstrumentSnapshot, session: SessionSnapshot) {
        for new_inst in instruments.instruments.iter_mut() {
            if let Some(old_inst) = self.instruments.instruments.iter().find(|i| i.id == new_inst.id) {
                if let (Some(old_seq), Some(new_seq)) = (&old_inst.drum_sequencer, &mut new_inst.drum_sequencer) {
                    if new_seq.playing {
                        new_seq.current_step = old_seq.current_step;
                        new_seq.step_accumulator = old_seq.step_accumulator;
                        new_seq.last_played_step = old_seq.last_played_step;
                    }
                }
            }
        }
        self.instruments = instruments;
        self.session = session;
    }

    fn apply_piano_roll_update(&mut self, updated: PianoRollSnapshot) {
        let playhead = self.piano_roll.playhead;
        let playing = self.piano_roll.playing;
        self.piano_roll = updated;
        self.piano_roll.playhead = playhead;
        self.piano_roll.playing = playing;
    }

    /// Resolve a VstTarget to a SuperCollider node ID using the instrument snapshot and engine node map
    fn resolve_vst_node_id(&self, instrument_id: InstrumentId, target: VstTarget) -> Option<i32> {
        let nodes = self.engine.node_map.get(&instrument_id)?;
        match target {
            VstTarget::Source => nodes.source,
            VstTarget::Effect(idx) => {
                // nodes.effects only contains enabled effects; map full index to enabled index
                let inst = self.instruments.instruments.iter()
                    .find(|i| i.id == instrument_id)?;
                let enabled_idx = inst.effects.iter()
                    .take(idx)
                    .filter(|e| e.enabled)
                    .count();
                nodes.effects.get(enabled_idx).copied()
            }
        }
    }

    /// Resolve the VstPluginId for a given instrument and target
    fn resolve_vst_plugin_id(&self, instrument_id: InstrumentId, target: VstTarget) -> Option<crate::state::vst_plugin::VstPluginId> {
        let inst = self.instruments.instruments.iter()
            .find(|i| i.id == instrument_id)?;
        match target {
            VstTarget::Source => {
                if let crate::state::SourceType::Vst(id) = inst.source {
                    Some(id)
                } else {
                    None
                }
            }
            VstTarget::Effect(idx) => {
                if let Some(effect) = inst.effects.get(idx) {
                    if let crate::state::EffectType::Vst(id) = effect.effect_type {
                        Some(id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }

    fn send_server_status(&self, status: ServerStatus, message: impl Into<String>) {
        let _ = self.feedback_tx.send(AudioFeedback::ServerStatus {
            status,
            message: message.into(),
            server_running: self.engine.server_running(),
        });
    }

    fn load_synthdefs_and_samples(&mut self) -> Result<(), String> {
        let synthdef_dir = Path::new("synthdefs");
        let builtin_result = self.engine.load_synthdefs(synthdef_dir);

        let config_dir = config_synthdefs_dir();
        let custom_result = if config_dir.exists() {
            self.engine.load_synthdefs(&config_dir)
        } else {
            Ok(())
        };

        // Initialize wavetable buffers for VOsc before any voices can play
        let _ = self.engine.initialize_wavetables();

        self.load_drum_samples();

        match (builtin_result, custom_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }

    fn load_drum_samples(&mut self) {
        for instrument in &self.instruments.instruments {
            if let Some(seq) = &instrument.drum_sequencer {
                for pad in &seq.pads {
                    if let (Some(buffer_id), Some(path)) = (pad.buffer_id, pad.path.as_ref()) {
                        let _ = self.engine.load_sample(buffer_id, path);
                    }
                }
            }
        }
    }

    /// Simple LCG random number generator, returns 0.0-1.0
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng_state >> 33) as f32) / (u32::MAX as f32)
    }

    fn tick(&mut self, elapsed: Duration) {
        self.tick_playback(elapsed);
        self.tick_drum_sequencer(elapsed);
        self.tick_arpeggiator(elapsed);
    }

    fn tick_playback(&mut self, elapsed: Duration) {
        let mut playback_data: Option<(
            Vec<(u32, u8, u8, u32, u32, f32)>, // instrument_id, pitch, velocity, duration, tick, probability
            u32,
            u32,
            u32,
            f64,
        )> = None;

        if self.piano_roll.playing {
            let seconds = elapsed.as_secs_f32();
            let ticks_f = seconds * (self.piano_roll.bpm / 60.0) * self.piano_roll.ticks_per_beat as f32;
            let tick_delta = ticks_f as u32;

            if tick_delta > 0 {
                let old_playhead = self.piano_roll.playhead;
                self.piano_roll.advance(tick_delta);
                let new_playhead = self.piano_roll.playhead;

                let (scan_start, scan_end) = if new_playhead >= old_playhead {
                    (old_playhead, new_playhead)
                } else {
                    (self.piano_roll.loop_start, new_playhead)
                };

                let secs_per_tick = 60.0 / (self.piano_roll.bpm as f64 * self.piano_roll.ticks_per_beat as f64);

                let mut note_ons: Vec<(u32, u8, u8, u32, u32, f32)> = Vec::new();
                for &instrument_id in &self.piano_roll.track_order {
                    if let Some(track) = self.piano_roll.tracks.get(&instrument_id) {
                        for note in &track.notes {
                            if note.tick >= scan_start && note.tick < scan_end {
                                note_ons.push((instrument_id, note.pitch, note.velocity, note.duration, note.tick, note.probability));
                            }
                        }
                    }
                }

                playback_data = Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick));
            }
        }

        if let Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick)) = playback_data {
            if self.engine.is_running() {
                let swing_amount = self.piano_roll.swing_amount;
                let humanize_vel = self.session.humanize_velocity;
                let humanize_time = self.session.humanize_timing;
                for &(instrument_id, pitch, velocity, duration, note_tick, probability) in &note_ons {
                    // Probability check: skip note if random exceeds probability
                    if probability < 1.0 && self.next_random() > probability {
                        continue;
                    }

                    // Check if this instrument has arpeggiator enabled
                    let arp_enabled = self.instruments.instruments.iter()
                        .find(|inst| inst.id == instrument_id)
                        .map(|inst| inst.arpeggiator.enabled)
                        .unwrap_or(false);

                    if arp_enabled {
                        // Buffer note for arpeggiator instead of spawning directly
                        let arp = self.arp_states.entry(instrument_id)
                            .or_insert_with(crate::state::arpeggiator::ArpPlayState::default);
                        if !arp.held_notes.contains(&pitch) {
                            arp.held_notes.push(pitch);
                            arp.held_notes.sort();
                        }
                        // Track as active note so note-off removes from held_notes
                        self.active_notes.push((instrument_id, pitch, duration));
                        continue;
                    }

                    let ticks_from_now = if note_tick >= old_playhead {
                        (note_tick - old_playhead) as f64
                    } else {
                        0.0
                    };
                    let mut offset = ticks_from_now * secs_per_tick;
                    // Apply swing: delay notes on offbeat positions (odd 8th notes)
                    if swing_amount > 0.0 {
                        let tpb = self.piano_roll.ticks_per_beat as f64;
                        let eighth = tpb / 2.0;
                        let pos_in_beat = (note_tick as f64) % tpb;
                        if (pos_in_beat - eighth).abs() < 1.0 {
                            offset += swing_amount as f64 * eighth * secs_per_tick * 0.5;
                        }
                    }
                    // Apply timing humanization: jitter offset by up to +/- 20ms
                    if humanize_time > 0.0 {
                        let jitter = (self.next_random() - 0.5) * 2.0 * humanize_time * 0.02;
                        offset = (offset + jitter as f64).max(0.0);
                    }
                    // Apply velocity humanization: jitter velocity by up to +/- 30
                    let mut vel_f = velocity as f32 / 127.0;
                    if humanize_vel > 0.0 {
                        let jitter = (self.next_random() - 0.5) * 2.0 * humanize_vel * (30.0 / 127.0);
                        vel_f = (vel_f + jitter).clamp(0.01, 1.0);
                    }
                    let _ = self.engine.spawn_voice(instrument_id, pitch, vel_f, offset, &self.instruments, &self.session);
                    self.active_notes.push((instrument_id, pitch, duration));
                }

                for lane in &self.automation_lanes {
                    if !lane.enabled {
                        continue;
                    }
                    if let Some(value) = lane.value_at(new_playhead) {
                        if matches!(lane.target, AutomationTarget::Bpm) {
                            if (self.piano_roll.bpm - value).abs() > f32::EPSILON {
                                self.piano_roll.bpm = value;
                                let _ = self.feedback_tx.send(AudioFeedback::BpmUpdate(value));
                            }
                        } else {
                            let _ = self.engine.apply_automation(&lane.target, value, &self.instruments, &self.session);
                        }
                    }
                }
            }

            let mut note_offs: Vec<(u32, u8, u32)> = Vec::new();
            for note in self.active_notes.iter_mut() {
                if note.2 <= tick_delta {
                    note_offs.push((note.0, note.1, note.2));
                    note.2 = 0;
                } else {
                    note.2 -= tick_delta;
                }
            }
            self.active_notes.retain(|n| n.2 > 0);

            if self.engine.is_running() {
                for (instrument_id, pitch, remaining) in &note_offs {
                    // For arp-enabled instruments, remove from held_notes instead of releasing
                    if let Some(arp) = self.arp_states.get_mut(instrument_id) {
                        arp.held_notes.retain(|&p| p != *pitch);
                        continue;
                    }
                    let offset = *remaining as f64 * secs_per_tick;
                    let _ = self.engine.release_voice(*instrument_id, *pitch, offset, &self.instruments);
                }
            }

            let _ = self.feedback_tx.send(AudioFeedback::PlayheadPosition(new_playhead));
        }
    }

    fn tick_drum_sequencer(&mut self, elapsed: Duration) {
        let bpm = self.piano_roll.bpm;

        for instrument in &mut self.instruments.instruments {
            let seq = match &mut instrument.drum_sequencer {
                Some(s) => s,
                None => continue,
            };
            if !seq.playing {
                seq.last_played_step = None;
                continue;
            }

            let pattern_length = seq.pattern().length;
            let steps_per_beat = 4.0_f32;
            let steps_per_second = (bpm / 60.0) * steps_per_beat;

            seq.step_accumulator += elapsed.as_secs_f32() * steps_per_second;

            // Swing: odd-numbered steps need a higher threshold to fire (delayed)
            let next_step = (seq.current_step + 1) % pattern_length;
            let swing_threshold = if seq.swing_amount > 0.0 && next_step % 2 == 1 {
                1.0 + seq.swing_amount * 0.5
            } else if seq.swing_amount > 0.0 && seq.current_step % 2 == 1 {
                // After a swung step, the following even step comes sooner
                1.0 - seq.swing_amount * 0.5
            } else {
                1.0
            };

            while seq.step_accumulator >= swing_threshold {
                seq.step_accumulator -= swing_threshold;
                let next = seq.current_step + 1;
                if next >= pattern_length {
                    // Pattern wrapped — advance chain if enabled
                    if seq.chain_enabled && !seq.chain.is_empty() {
                        seq.chain_position = (seq.chain_position + 1) % seq.chain.len();
                        let next_pattern = seq.chain[seq.chain_position];
                        if next_pattern < seq.patterns.len() {
                            seq.current_pattern = next_pattern;
                        }
                    }
                    seq.current_step = 0;
                } else {
                    seq.current_step = next;
                }
            }

            if seq.last_played_step != Some(seq.current_step) {
                if self.engine.is_running() && !instrument.mute {
                    let current_step = seq.current_step;
                    let current_pattern = seq.current_pattern;
                    let pattern = &seq.patterns[current_pattern];
                    for (pad_idx, pad) in seq.pads.iter().enumerate() {
                        if let Some(buffer_id) = pad.buffer_id {
                            if let Some(step) = pattern
                                .steps
                                .get(pad_idx)
                                .and_then(|s| s.get(current_step))
                            {
                                if step.active {
                                    // Probability check: skip hit if random exceeds probability
                                    // (inline RNG to avoid &mut self borrow conflict)
                                    if step.probability < 1.0 {
                                        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                                        let r = ((self.rng_state >> 33) as f32) / (u32::MAX as f32);
                                        if r > step.probability { continue; }
                                    }
                                    let mut amp = (step.velocity as f32 / 127.0) * pad.level;
                                    // Velocity humanization
                                    if self.session.humanize_velocity > 0.0 {
                                        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                                        let r = ((self.rng_state >> 33) as f32) / (u32::MAX as f32);
                                        let jitter = (r - 0.5) * 2.0 * self.session.humanize_velocity * (30.0 / 127.0);
                                        amp = (amp + jitter).clamp(0.01, 1.0);
                                    }
                                    let _ = self.engine.play_drum_hit_to_instrument(
                                        buffer_id, amp, instrument.id,
                                        pad.slice_start, pad.slice_end,
                                    );
                                }
                            }
                        }
                    }
                }
                let _ = self.feedback_tx.send(AudioFeedback::DrumSequencerStep {
                    instrument_id: instrument.id,
                    step: seq.current_step,
                });
                seq.last_played_step = Some(seq.current_step);
            }
        }
    }

    fn tick_arpeggiator(&mut self, elapsed: Duration) {
        use crate::state::arpeggiator::{ArpDirection, ArpPlayState};

        let bpm = self.piano_roll.bpm;

        // Collect instrument ids and arp configs to avoid borrow conflicts
        let arp_instruments: Vec<(u32, crate::state::arpeggiator::ArpeggiatorConfig)> = self
            .instruments
            .instruments
            .iter()
            .filter(|inst| inst.arpeggiator.enabled)
            .map(|inst| (inst.id, inst.arpeggiator.clone()))
            .collect();

        for (instrument_id, config) in arp_instruments {
            let arp = self.arp_states.entry(instrument_id).or_insert_with(ArpPlayState::default);

            if arp.held_notes.is_empty() {
                // Release any currently sounding note
                if let Some(pitch) = arp.current_pitch.take() {
                    if self.engine.is_running() {
                        let _ = self.engine.release_voice(instrument_id, pitch, 0.0, &self.instruments);
                    }
                }
                continue;
            }

            // Build the note sequence: held notes across octaves
            let mut sequence: Vec<u8> = Vec::new();
            for octave in 0..config.octaves {
                for &note in &arp.held_notes {
                    let pitched = note as i16 + (octave as i16 * 12);
                    if (0..=127).contains(&pitched) {
                        sequence.push(pitched as u8);
                    }
                }
            }
            if sequence.is_empty() {
                continue;
            }

            // Advance accumulator
            let steps_per_second = (bpm / 60.0) * config.rate.steps_per_beat();
            arp.accumulator += elapsed.as_secs_f32() * steps_per_second;

            while arp.accumulator >= 1.0 {
                arp.accumulator -= 1.0;

                // Release previous note
                if let Some(pitch) = arp.current_pitch.take() {
                    if self.engine.is_running() {
                        let _ = self.engine.release_voice(instrument_id, pitch, 0.0, &self.instruments);
                    }
                }

                // Select next note based on direction
                let seq_len = sequence.len();
                let pitch = match config.direction {
                    ArpDirection::Up => {
                        arp.step_index = (arp.step_index + 1) % seq_len;
                        sequence[arp.step_index]
                    }
                    ArpDirection::Down => {
                        if arp.step_index == 0 {
                            arp.step_index = seq_len - 1;
                        } else {
                            arp.step_index -= 1;
                        }
                        sequence[arp.step_index]
                    }
                    ArpDirection::UpDown => {
                        if seq_len <= 1 {
                            sequence[0]
                        } else {
                            if arp.ascending {
                                arp.step_index += 1;
                                if arp.step_index >= seq_len {
                                    arp.step_index = seq_len - 2;
                                    arp.ascending = false;
                                }
                            } else {
                                if arp.step_index == 0 {
                                    arp.step_index = 1;
                                    arp.ascending = true;
                                } else {
                                    arp.step_index -= 1;
                                }
                            }
                            sequence[arp.step_index.min(seq_len - 1)]
                        }
                    }
                    ArpDirection::Random => {
                        // Use inline RNG to pick random index
                        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        let r = ((self.rng_state >> 33) as usize) % seq_len;
                        sequence[r]
                    }
                };

                // Spawn the new note
                if self.engine.is_running() {
                    let vel_f = 0.8; // Default velocity for arp notes
                    let _ = self.engine.spawn_voice(instrument_id, pitch, vel_f, 0.0, &self.instruments, &self.session);
                }
                arp.current_pitch = Some(pitch);
            }
        }

        // Clean up arp states for instruments that no longer have arp enabled
        let active_ids: Vec<u32> = self
            .instruments
            .instruments
            .iter()
            .filter(|inst| inst.arpeggiator.enabled)
            .map(|inst| inst.id)
            .collect();
        self.arp_states.retain(|id, state| {
            if !active_ids.contains(id) {
                // Release any sounding note before removing
                if let Some(pitch) = state.current_pitch {
                    if self.engine.is_running() {
                        let _ = self.engine.release_voice(*id, pitch, 0.0, &self.instruments);
                    }
                }
                false
            } else {
                true
            }
        });
    }

    fn poll_engine(&mut self) {
        if let Some(result) = self.engine.poll_compile_result() {
            let result = match result {
                Ok(msg) => {
                    // Auto-reload synthdefs after successful compile
                    let mut reload_msg = msg;
                    let builtin_dir = Path::new("synthdefs");
                    if builtin_dir.exists() {
                        match self.engine.load_synthdefs(builtin_dir) {
                            Ok(()) => reload_msg += " — reloaded",
                            Err(e) => reload_msg += &format!(" — reload failed: {e}"),
                        }
                    }
                    // Also reload custom synthdefs from config dir
                    if let Some(home) = std::env::var_os("HOME") {
                        let config_dir = std::path::PathBuf::from(home)
                            .join(".config/imbolc/synthdefs");
                        if config_dir.exists() {
                            let _ = self.engine.load_synthdefs(&config_dir);
                        }
                    }
                    Ok(reload_msg)
                }
                Err(e) => Err(e),
            };
            let _ = self.feedback_tx.send(AudioFeedback::CompileResult(result));
        }

        if let Some(msg) = self.engine.check_server_health() {
            let _ = self.feedback_tx.send(AudioFeedback::ServerStatus {
                status: self.engine.status(),
                message: msg,
                server_running: self.engine.server_running(),
            });
        }

        if self.engine.poll_pending_buffer_free() {
            let _ = self.feedback_tx.send(AudioFeedback::PendingBufferFreed);
        }

        let is_recording = self.engine.is_recording();
        let elapsed_secs = self
            .engine
            .recording_elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if is_recording != self.last_recording_state || (is_recording && elapsed_secs != self.last_recording_secs) {
            self.last_recording_state = is_recording;
            self.last_recording_secs = elapsed_secs;
            let _ = self.feedback_tx.send(AudioFeedback::RecordingState {
                is_recording,
                elapsed_secs,
            });
        }
    }
}
