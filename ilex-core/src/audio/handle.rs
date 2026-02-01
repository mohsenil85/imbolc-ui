//! AudioHandle: main-thread interface to the audio engine.
//!
//! Wraps AudioEngine and owns playback state (active_notes, playback ticking).
//! Phase 3: Methods will send AudioCmd through an MPSC channel to a dedicated
//! audio thread, and state queries will read from cached feedback values.

use std::path::Path;
use std::time::Duration;

use super::engine::AudioEngine;
use super::ServerStatus;
use crate::state::automation::AutomationTarget;
use crate::state::{AppState, BufferId, InstrumentId, InstrumentState, SessionState};

/// Main-thread handle to the audio subsystem.
///
/// Owns the AudioEngine, active_notes tracking, and playback ticking.
/// In Phase 3 this will hold an mpsc::Sender<AudioCmd> and mpsc::Receiver<AudioFeedback>.
pub struct AudioHandle {
    engine: AudioEngine,
    /// Active notes: (instrument_id, pitch, remaining_ticks)
    active_notes: Vec<(u32, u8, u32)>,
}

impl AudioHandle {
    pub fn new() -> Self {
        Self {
            engine: AudioEngine::new(),
            active_notes: Vec::new(),
        }
    }

    // ── State accessors ───────────────────────────────────────────

    pub fn is_running(&self) -> bool {
        self.engine.is_running()
    }

    pub fn status(&self) -> ServerStatus {
        self.engine.status()
    }

    pub fn server_running(&self) -> bool {
        self.engine.server_running()
    }

    #[allow(dead_code)]
    pub fn is_compiling(&self) -> bool {
        self.engine.is_compiling()
    }

    pub fn master_peak(&self) -> f32 {
        self.engine.master_peak()
    }

    pub fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.engine.audio_in_waveform(instrument_id)
    }

    pub fn is_recording(&self) -> bool {
        self.engine.is_recording()
    }

    pub fn recording_elapsed(&self) -> Option<Duration> {
        self.engine.recording_elapsed()
    }

    // ── Server lifecycle ──────────────────────────────────────────

    pub fn connect(&mut self, server_addr: &str) -> std::io::Result<()> {
        self.engine.connect(server_addr)
    }

    pub fn disconnect(&mut self) {
        self.engine.disconnect()
    }

    pub fn start_server_with_devices(
        &mut self,
        input_device: Option<&str>,
        output_device: Option<&str>,
    ) -> Result<(), String> {
        self.engine.start_server_with_devices(input_device, output_device)
    }

    pub fn stop_server(&mut self) {
        self.engine.stop_server()
    }

    pub fn compile_synthdefs_async(&mut self, scd_path: &Path) -> Result<(), String> {
        self.engine.compile_synthdefs_async(scd_path)
    }

    pub fn poll_compile_result(&mut self) -> Option<Result<String, String>> {
        self.engine.poll_compile_result()
    }

    pub fn check_server_health(&mut self) -> Option<String> {
        self.engine.check_server_health()
    }

    // ── SynthDefs & samples ───────────────────────────────────────

    pub fn load_synthdefs(&self, dir: &Path) -> Result<(), String> {
        self.engine.load_synthdefs(dir)
    }

    pub fn load_synthdef_file(&self, path: &Path) -> Result<(), String> {
        self.engine.load_synthdef_file(path)
    }

    pub fn load_sample(&mut self, buffer_id: BufferId, path: &str) -> Result<i32, String> {
        self.engine.load_sample(buffer_id, path)
    }

    // ── Routing & mixing ──────────────────────────────────────────

    pub fn rebuild_instrument_routing(
        &mut self,
        instruments: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        self.engine.rebuild_instrument_routing(instruments, session)
    }

    pub fn set_bus_mixer_params(
        &self,
        bus_id: u8,
        level: f32,
        mute: bool,
        pan: f32,
    ) -> Result<(), String> {
        self.engine.set_bus_mixer_params(bus_id, level, mute, pan)
    }

    pub fn update_all_instrument_mixer_params(
        &self,
        instruments: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        self.engine.update_all_instrument_mixer_params(instruments, session)
    }

    pub fn set_source_param(
        &self,
        instrument_id: InstrumentId,
        param: &str,
        value: f32,
    ) -> Result<(), String> {
        self.engine.set_source_param(instrument_id, param, value)
    }

    // ── Voice management ──────────────────────────────────────────

    pub fn spawn_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        instruments: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        self.engine.spawn_voice(instrument_id, pitch, velocity, offset_secs, instruments, session)
    }

    pub fn release_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        offset_secs: f64,
        instruments: &InstrumentState,
    ) -> Result<(), String> {
        self.engine.release_voice(instrument_id, pitch, offset_secs, instruments)
    }

    pub fn release_all_voices(&mut self) {
        self.engine.release_all_voices()
    }

    pub fn play_drum_hit_to_instrument(
        &mut self,
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
    ) -> Result<(), String> {
        self.engine.play_drum_hit_to_instrument(buffer_id, amp, instrument_id, slice_start, slice_end)
    }

    // ── Recording ─────────────────────────────────────────────────

    pub fn start_recording(&mut self, bus: i32, path: &Path) -> Result<(), String> {
        self.engine.start_recording(bus, path)
    }

    pub fn stop_recording(&mut self) -> Option<std::path::PathBuf> {
        self.engine.stop_recording()
    }

    pub fn poll_pending_buffer_free(&mut self) -> bool {
        self.engine.poll_pending_buffer_free()
    }

    // ── Automation ────────────────────────────────────────────────

    pub fn apply_automation(
        &self,
        target: &AutomationTarget,
        value: f32,
        instruments: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        self.engine.apply_automation(target, value, instruments, session)
    }

    // ── Active notes (owned by AudioHandle) ───────────────────────

    /// Register an active note for automatic release after `duration_ticks`.
    pub fn push_active_note(&mut self, instrument_id: u32, pitch: u8, duration_ticks: u32) {
        self.active_notes.push((instrument_id, pitch, duration_ticks));
    }

    /// Clear all active notes (e.g. on play/stop).
    pub fn clear_active_notes(&mut self) {
        self.active_notes.clear();
    }

    // ── Playback tick ─────────────────────────────────────────────

    /// Combined playback tick: advances piano roll playhead, processes note-on/off
    /// events, applies automation, and ticks drum sequencers.
    ///
    /// Call this once per frame from the main loop with the elapsed Duration.
    pub fn tick(&mut self, state: &mut AppState, elapsed: Duration) {
        self.tick_playback(state, elapsed);
        self.tick_drum_sequencer(state, elapsed);
    }

    /// Advance the piano roll playhead and process note-on/off events.
    fn tick_playback(&mut self, state: &mut AppState, elapsed: Duration) {
        // Phase 1: advance playhead and collect note events
        let mut playback_data: Option<(
            Vec<(u32, u8, u8, u32, u32)>, // note_ons: (instrument_id, pitch, vel, duration, tick)
            u32,                           // old_playhead
            u32,                           // new_playhead
            u32,                           // tick_delta
            f64,                           // secs_per_tick
        )> = None;

        {
            let pr = &mut state.session.piano_roll;
            if pr.playing {
                let seconds = elapsed.as_secs_f32();
                let ticks_f = seconds * (pr.bpm / 60.0) * pr.ticks_per_beat as f32;
                let tick_delta = ticks_f as u32;

                if tick_delta > 0 {
                    let old_playhead = pr.playhead;
                    pr.advance(tick_delta);
                    let new_playhead = pr.playhead;

                    let (scan_start, scan_end) = if new_playhead >= old_playhead {
                        (old_playhead, new_playhead)
                    } else {
                        (pr.loop_start, new_playhead)
                    };

                    let secs_per_tick = 60.0 / (pr.bpm as f64 * pr.ticks_per_beat as f64);

                    let mut note_ons: Vec<(u32, u8, u8, u32, u32)> = Vec::new();
                    for &instrument_id in &pr.track_order {
                        if let Some(track) = pr.tracks.get(&instrument_id) {
                            for note in &track.notes {
                                if note.tick >= scan_start && note.tick < scan_end {
                                    note_ons.push((instrument_id, note.pitch, note.velocity, note.duration, note.tick));
                                }
                            }
                        }
                    }

                    playback_data = Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick));
                }
            }
        }

        // Phase 2: send note-ons/offs and process automation
        if let Some((note_ons, old_playhead, new_playhead, tick_delta, secs_per_tick)) = playback_data {
            if self.engine.is_running() {
                // Process note-ons
                for &(instrument_id, pitch, velocity, duration, note_tick) in &note_ons {
                    let ticks_from_now = if note_tick >= old_playhead {
                        (note_tick - old_playhead) as f64
                    } else {
                        0.0
                    };
                    let offset = ticks_from_now * secs_per_tick;
                    let vel_f = velocity as f32 / 127.0;
                    let _ = self.engine.spawn_voice(instrument_id, pitch, vel_f, offset, &state.instruments, &state.session);
                    self.active_notes.push((instrument_id, pitch, duration));
                }

                // Process automation
                for lane in &state.session.automation.lanes {
                    if !lane.enabled {
                        continue;
                    }
                    if let Some(value) = lane.value_at(new_playhead) {
                        if matches!(lane.target, AutomationTarget::Bpm) {
                            state.session.piano_roll.bpm = value;
                        } else {
                            let _ = self.engine.apply_automation(&lane.target, value, &state.instruments, &state.session);
                        }
                    }
                }
            }

            // Process active notes: decrement remaining ticks, send note-offs
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
                    let offset = *remaining as f64 * secs_per_tick;
                    let _ = self.engine.release_voice(*instrument_id, *pitch, offset, &state.instruments);
                }
            }
        }
    }

    /// Advance the drum sequencer for each drum machine instrument and trigger pad hits.
    fn tick_drum_sequencer(&mut self, state: &mut AppState, elapsed: Duration) {
        let bpm = state.session.piano_roll.bpm;

        for instrument in &mut state.instruments.instruments {
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

            while seq.step_accumulator >= 1.0 {
                seq.step_accumulator -= 1.0;
                seq.current_step = (seq.current_step + 1) % pattern_length;
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
                                    let amp = (step.velocity as f32 / 127.0) * pad.level;
                                    let _ = self.engine.play_drum_hit_to_instrument(
                                        buffer_id, amp, instrument.id,
                                        pad.slice_start, pad.slice_end,
                                    );
                                }
                            }
                        }
                    }
                }
                seq.last_played_step = Some(seq.current_step);
            }
        }
    }
}

impl Default for AudioHandle {
    fn default() -> Self {
        Self::new()
    }
}
