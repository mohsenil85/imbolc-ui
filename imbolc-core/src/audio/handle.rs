//! AudioHandle: main-thread interface to the audio engine.
//!
//! Owns the command/feedback channels and shared monitor state. The
//! AudioEngine and playback ticking live on the audio thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::commands::{AudioCmd, AudioFeedback};
use super::osc_client::AudioMonitor;
use super::snapshot::{AutomationSnapshot, InstrumentSnapshot, PianoRollSnapshot, SessionSnapshot};
use super::ServerStatus;
use crate::action::AudioDirty;
use crate::state::automation::AutomationTarget;
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

impl AudioHandle {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (feedback_tx, feedback_rx) = mpsc::channel();
        let monitor = AudioMonitor::new();
        let thread_monitor = monitor.clone();

        let join_handle = thread::spawn(move || {
            let thread = super::audio_thread::AudioThread::new(cmd_rx, feedback_tx, thread_monitor);
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

        let needs_full_state = dirty.instruments || dirty.session || dirty.routing;
        if needs_full_state {
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
            if needs_full_state {
                // Full state already sent — just trigger the engine update
                let _ = self.send_cmd(AudioCmd::UpdateMixerParams);
            } else {
                // Mixer-only change: send targeted updates (no full clone)
                self.send_mixer_params_incremental(state);
            }
        }
    }

    fn send_mixer_params_incremental(&self, state: &AppState) {
        let _ = self.send_cmd(AudioCmd::SetMasterParams {
            level: state.session.master_level,
            mute: state.session.master_mute,
        });
        for inst in &state.instruments.instruments {
            let _ = self.send_cmd(AudioCmd::SetInstrumentMixerParams {
                instrument_id: inst.id,
                level: inst.level,
                pan: inst.pan,
                mute: inst.mute,
                solo: inst.solo,
            });
        }
        // After all fields are updated on the audio thread, trigger engine apply
        let _ = self.send_cmd(AudioCmd::UpdateMixerParams);
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

    pub fn stop_recording(&mut self) -> Option<PathBuf> {
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
