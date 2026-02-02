use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::commands::{AudioCmd, AudioFeedback};
use super::engine::AudioEngine;
use super::osc_client::AudioMonitor;
use super::ServerStatus;
use crate::action::VstTarget;
use crate::state::arpeggiator::ArpPlayState;
use super::snapshot::{AutomationSnapshot, InstrumentSnapshot, PianoRollSnapshot, SessionSnapshot};
use crate::state::{InstrumentState, SessionState};

pub(crate) struct AudioThread {
    engine: AudioEngine,
    cmd_rx: Receiver<AudioCmd>,
    feedback_tx: Sender<AudioFeedback>,
    monitor: AudioMonitor,
    instruments: InstrumentSnapshot,
    session: SessionSnapshot,
    piano_roll: PianoRollSnapshot,
    automation_lanes: AutomationSnapshot,
    active_notes: Vec<(u32, u8, u32)>, // (instrument_id, pitch, duration_ticks)
    last_tick: Instant,
    last_recording_secs: u64,
    last_recording_state: bool,
    /// Simple LCG random seed for probability/humanization
    rng_state: u64,
    /// Per-instrument arpeggiator runtime state
    arp_states: HashMap<u32, ArpPlayState>,
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

impl AudioThread {
    pub(crate) fn new(
        cmd_rx: Receiver<AudioCmd>,
        feedback_tx: Sender<AudioFeedback>,
        monitor: AudioMonitor,
    ) -> Self {
        Self {
            engine: AudioEngine::new(),
            cmd_rx,
            feedback_tx,
            monitor,
            instruments: InstrumentState::new(),
            session: SessionState::new(),
            piano_roll: PianoRollSnapshot::new(),
            automation_lanes: Vec::new(),
            active_notes: Vec::new(),
            last_tick: Instant::now(),
            last_recording_secs: 0,
            last_recording_state: false,
            rng_state: 12345,
            arp_states: HashMap::new(),
        }
    }

    pub(crate) fn run(mut self) {
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
    fn resolve_vst_node_id(&self, instrument_id: u32, target: VstTarget) -> Option<i32> {
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
    fn resolve_vst_plugin_id(&self, instrument_id: u32, target: VstTarget) -> Option<crate::state::vst_plugin::VstPluginId> {
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

    fn tick(&mut self, elapsed: Duration) {
        super::playback::tick_playback(
            &mut self.piano_roll,
            &self.instruments,
            &self.session,
            &self.automation_lanes,
            &mut self.engine,
            &mut self.active_notes,
            &mut self.arp_states,
            &mut self.rng_state,
            &self.feedback_tx,
            elapsed,
        );
        super::drum_tick::tick_drum_sequencer(
            &mut self.instruments,
            &self.session,
            self.piano_roll.bpm,
            &mut self.engine,
            &mut self.rng_state,
            &self.feedback_tx,
            elapsed,
        );
        super::arpeggiator_tick::tick_arpeggiator(
            &self.instruments,
            &self.session,
            self.piano_roll.bpm,
            &mut self.arp_states,
            &mut self.engine,
            &mut self.rng_state,
            elapsed,
        );
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
