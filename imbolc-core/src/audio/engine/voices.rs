use std::time::Instant;

use super::{AudioEngine, VoiceChain, MAX_VOICES_PER_INSTRUMENT, GROUP_SOURCES};
use crate::state::{BufferId, InstrumentId, InstrumentState, LfoTarget, ParamValue, SessionState};

impl AudioEngine {
    /// Spawn a voice for an instrument
    pub fn spawn_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        state: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        let instrument = state.instrument(instrument_id)
            .ok_or_else(|| format!("No instrument with id {}", instrument_id))?;

        // AudioIn, BusIn, and VSTi instruments don't use voice spawning - they have persistent synths
        if instrument.source.is_audio_input() || instrument.source.is_bus_in() {
            return Ok(());
        }

        // VSTi instruments: send MIDI note-on via /u_cmd
        if instrument.source.is_vst() {
            return self.send_vsti_note_on(instrument_id, pitch, velocity);
        }

        // Sampler instruments need special handling
        if instrument.source.is_sample() {
            return self.spawn_sampler_voice(instrument_id, pitch, velocity, offset_secs, state, session);
        }

        // Smart voice stealing (must precede client borrow)
        self.steal_voice_if_needed(instrument_id, pitch, velocity)?;

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(instrument_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_node_id;
        self.next_node_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let tuning = session.tuning_a4 as f64;
        let freq = tuning * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        let mut messages: Vec<rosc::OscMessage> = Vec::new();

        // 1. Create group
        messages.push(rosc::OscMessage {
            addr: "/g_new".to_string(),
            args: vec![
                rosc::OscType::Int(group_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(GROUP_SOURCES),
            ],
        });

        // 2. MIDI control node
        let midi_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("imbolc_midi".to_string()),
                rosc::OscType::Int(midi_node_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(group_id),
            ];
            let params: Vec<(String, f32)> = vec![
                ("note".to_string(), pitch as f32),
                ("freq".to_string(), freq as f32),
                ("vel".to_string(), velocity),
                ("gate".to_string(), 1.0),
                ("freq_out".to_string(), voice_freq_bus as f32),
                ("gate_out".to_string(), voice_gate_bus as f32),
                ("vel_out".to_string(), voice_vel_bus as f32),
            ];
            for (name, value) in &params {
                args.push(rosc::OscType::String(name.clone()));
                args.push(rosc::OscType::Float(*value));
            }
            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // 3. Source synth
        let source_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String(Self::source_synth_def(instrument.source, &session.custom_synthdefs)),
                rosc::OscType::Int(source_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];
            // Source params
            for p in &instrument.source_params {
                let val = match &p.value {
                    ParamValue::Float(v) => *v,
                    ParamValue::Int(v) => *v as f32,
                    ParamValue::Bool(v) => if *v { 1.0 } else { 0.0 },
                };
                args.push(rosc::OscType::String(p.name.clone()));
                args.push(rosc::OscType::Float(val));
            }
            // Wire control inputs
            args.push(rosc::OscType::String("freq_in".to_string()));
            args.push(rosc::OscType::Float(voice_freq_bus as f32));
            args.push(rosc::OscType::String("gate_in".to_string()));
            args.push(rosc::OscType::Float(voice_gate_bus as f32));
            // Amp envelope (ADSR)
            args.push(rosc::OscType::String("attack".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.release));
            // Output to source_out_bus
            args.push(rosc::OscType::String("out".to_string()));
            args.push(rosc::OscType::Float(source_out_bus as f32));

            // Wire LFO mod inputs based on target
            if instrument.lfo.enabled {
                if let Some(lfo_bus) = self.bus_allocator.get_control_bus(instrument_id, "lfo_out") {
                    match instrument.lfo.target {
                        LfoTarget::Amplitude => {
                            args.push(rosc::OscType::String("amp_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Pitch => {
                            args.push(rosc::OscType::String("pitch_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Detune => {
                            args.push(rosc::OscType::String("detune_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::PulseWidth => {
                            args.push(rosc::OscType::String("width_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Attack => {
                            args.push(rosc::OscType::String("attack_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Release => {
                            args.push(rosc::OscType::String("release_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::FmIndex => {
                            args.push(rosc::OscType::String("index_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::WavetablePosition => {
                            args.push(rosc::OscType::String("position_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::FormantFreq => {
                            args.push(rosc::OscType::String("formant_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::SyncRatio => {
                            args.push(rosc::OscType::String("sync_ratio_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Pressure => {
                            args.push(rosc::OscType::String("pressure_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Embouchure => {
                            args.push(rosc::OscType::String("embouchure_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::GrainSize => {
                            args.push(rosc::OscType::String("grain_size_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::GrainDensity => {
                            args.push(rosc::OscType::String("density_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::FbFeedback => {
                            args.push(rosc::OscType::String("feedback_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::RingModDepth => {
                            args.push(rosc::OscType::String("mod_depth_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::ChaosParam => {
                            args.push(rosc::OscType::String("chaos_param_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::AdditiveRolloff => {
                            args.push(rosc::OscType::String("rolloff_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::MembraneTension => {
                            args.push(rosc::OscType::String("tension_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Decay => {
                            args.push(rosc::OscType::String("decay_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Sustain => {
                            args.push(rosc::OscType::String("sustain_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        _ => {} // Routing-level targets handled in routing.rs
                    }
                }
            }

            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // Send all as one timed bundle
        let time = super::super::osc_client::osc_time_from_now(offset_secs);
        client
            .send_bundle(messages, time)
            .map_err(|e| e.to_string())?;

        self.voice_chains.push(VoiceChain {
            instrument_id,
            pitch,
            velocity,
            group_id,
            midi_node_id,
            source_node: source_node_id,
            spawn_time: Instant::now(),
            release_state: None,
        });

        Ok(())
    }

    /// Spawn a sampler voice (separate method for sampler-specific handling)
    fn spawn_sampler_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        velocity: f32,
        offset_secs: f64,
        state: &InstrumentState,
        session: &SessionState,
    ) -> Result<(), String> {
        let instrument = state.instrument(instrument_id)
            .ok_or_else(|| format!("No instrument with id {}", instrument_id))?;

        let sampler_config = instrument.sampler_config.as_ref()
            .ok_or("Sampler instrument has no sampler config")?;

        let buffer_id = sampler_config.buffer_id
            .ok_or("Sampler has no buffer loaded")?;

        let bufnum = self.buffer_map.get(&buffer_id)
            .copied()
            .ok_or("Buffer not loaded in audio engine")?;

        // Get slice for this note (or current selected slice)
        let (slice_start, slice_end) = sampler_config.slice_for_note(pitch)
            .map(|s| (s.start, s.end))
            .unwrap_or((0.0, 1.0));

        // Smart voice stealing (must precede client borrow)
        self.steal_voice_if_needed(instrument_id, pitch, velocity)?;

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Get the audio bus where voices should write their output
        let source_out_bus = self.bus_allocator.get_audio_bus(instrument_id, "source_out").unwrap_or(16);

        // Create a group for this voice chain
        let group_id = self.next_node_id;
        self.next_node_id += 1;

        // Allocate per-voice control buses
        let voice_freq_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_gate_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;
        let voice_vel_bus = self.next_voice_control_bus;
        self.next_voice_control_bus += 1;

        let tuning = session.tuning_a4 as f64;
        let freq = tuning * (2.0_f64).powf((pitch as f64 - 69.0) / 12.0);

        let mut messages: Vec<rosc::OscMessage> = Vec::new();

        // 1. Create group
        messages.push(rosc::OscMessage {
            addr: "/g_new".to_string(),
            args: vec![
                rosc::OscType::Int(group_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(GROUP_SOURCES),
            ],
        });

        // 2. MIDI control node
        let midi_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("imbolc_midi".to_string()),
                rosc::OscType::Int(midi_node_id),
                rosc::OscType::Int(1), // addToTail
                rosc::OscType::Int(group_id),
            ];
            let params: Vec<(String, f32)> = vec![
                ("note".to_string(), pitch as f32),
                ("freq".to_string(), freq as f32),
                ("vel".to_string(), velocity),
                ("gate".to_string(), 1.0),
                ("freq_out".to_string(), voice_freq_bus as f32),
                ("gate_out".to_string(), voice_gate_bus as f32),
                ("vel_out".to_string(), voice_vel_bus as f32),
            ];
            for (name, value) in &params {
                args.push(rosc::OscType::String(name.clone()));
                args.push(rosc::OscType::Float(*value));
            }
            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // 3. Sampler synth
        let sampler_node_id = self.next_node_id;
        self.next_node_id += 1;
        {
            let mut args: Vec<rosc::OscType> = vec![
                rosc::OscType::String("imbolc_sampler".to_string()),
                rosc::OscType::Int(sampler_node_id),
                rosc::OscType::Int(1),
                rosc::OscType::Int(group_id),
            ];

            // Get rate and amp from source params
            let rate = instrument.source_params.iter()
                .find(|p| p.name == "rate")
                .map(|p| match &p.value {
                    ParamValue::Float(v) => *v,
                    _ => 1.0,
                })
                .unwrap_or(1.0);

            let amp = instrument.source_params.iter()
                .find(|p| p.name == "amp")
                .map(|p| match &p.value {
                    ParamValue::Float(v) => *v,
                    _ => 0.8,
                })
                .unwrap_or(0.8);

            let loop_mode = sampler_config.loop_mode;

            // Sampler params
            args.push(rosc::OscType::String("bufnum".to_string()));
            args.push(rosc::OscType::Float(bufnum as f32));
            args.push(rosc::OscType::String("sliceStart".to_string()));
            args.push(rosc::OscType::Float(slice_start));
            args.push(rosc::OscType::String("sliceEnd".to_string()));
            args.push(rosc::OscType::Float(slice_end));
            args.push(rosc::OscType::String("rate".to_string()));
            args.push(rosc::OscType::Float(rate));
            args.push(rosc::OscType::String("amp".to_string()));
            args.push(rosc::OscType::Float(amp));
            args.push(rosc::OscType::String("loop".to_string()));
            args.push(rosc::OscType::Float(if loop_mode { 1.0 } else { 0.0 }));

            // Wire control inputs (for pitch tracking if enabled)
            if sampler_config.pitch_tracking {
                args.push(rosc::OscType::String("freq_in".to_string()));
                args.push(rosc::OscType::Float(voice_freq_bus as f32));
            }
            args.push(rosc::OscType::String("gate_in".to_string()));
            args.push(rosc::OscType::Float(voice_gate_bus as f32));
            args.push(rosc::OscType::String("vel_in".to_string()));
            args.push(rosc::OscType::Float(voice_vel_bus as f32));

            // Amp envelope (ADSR)
            args.push(rosc::OscType::String("attack".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.attack));
            args.push(rosc::OscType::String("decay".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.decay));
            args.push(rosc::OscType::String("sustain".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.sustain));
            args.push(rosc::OscType::String("release".to_string()));
            args.push(rosc::OscType::Float(instrument.amp_envelope.release));

            // Output to source_out_bus
            args.push(rosc::OscType::String("out".to_string()));
            args.push(rosc::OscType::Float(source_out_bus as f32));

            // Wire LFO mod inputs for sampler voice
            if instrument.lfo.enabled {
                if let Some(lfo_bus) = self.bus_allocator.get_control_bus(instrument_id, "lfo_out") {
                    match instrument.lfo.target {
                        LfoTarget::Amplitude => {
                            args.push(rosc::OscType::String("amp_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::SampleRate => {
                            args.push(rosc::OscType::String("srate_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Attack => {
                            args.push(rosc::OscType::String("attack_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Release => {
                            args.push(rosc::OscType::String("release_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Decay => {
                            args.push(rosc::OscType::String("decay_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        LfoTarget::Sustain => {
                            args.push(rosc::OscType::String("sustain_mod_in".to_string()));
                            args.push(rosc::OscType::Float(lfo_bus as f32));
                        }
                        _ => {} // Routing-level targets handled in routing.rs
                    }
                }
            }

            messages.push(rosc::OscMessage {
                addr: "/s_new".to_string(),
                args,
            });
        }

        // Send all as one timed bundle
        let time = super::super::osc_client::osc_time_from_now(offset_secs);
        client
            .send_bundle(messages, time)
            .map_err(|e| e.to_string())?;

        self.voice_chains.push(VoiceChain {
            instrument_id,
            pitch,
            velocity,
            group_id,
            midi_node_id,
            source_node: sampler_node_id,
            spawn_time: Instant::now(),
            release_state: None,
        });

        Ok(())
    }

    /// Release a specific voice by instrument and pitch (note-off).
    /// Marks the voice as released instead of removing it, so it remains
    /// available as a steal candidate while its envelope fades out.
    pub fn release_voice(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        offset_secs: f64,
        state: &InstrumentState,
    ) -> Result<(), String> {
        // VSTi instruments: send MIDI note-off via /u_cmd
        if let Some(instrument) = state.instrument(instrument_id) {
            if instrument.source.is_vst() {
                return self.send_vsti_note_off(instrument_id, pitch);
            }
        }

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Find an active (non-released) voice matching this instrument and pitch
        if let Some(pos) = self
            .voice_chains
            .iter()
            .position(|v| {
                v.instrument_id == instrument_id
                    && v.pitch == pitch
                    && v.release_state.is_none()
            })
        {
            let release_time = state.instrument(instrument_id)
                .map(|s| s.amp_envelope.release)
                .unwrap_or(1.0);

            // Send gate=0 to begin envelope release
            let time = super::super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(
                    self.voice_chains[pos].midi_node_id,
                    &[("gate", 0.0)],
                    time,
                )
                .map_err(|e| e.to_string())?;

            // Schedule deferred /n_free after envelope completes (+1s margin)
            let cleanup_time = super::super::osc_client::osc_time_from_now(
                offset_secs + release_time as f64 + 1.0,
            );
            client
                .send_bundle(
                    vec![rosc::OscMessage {
                        addr: "/n_free".to_string(),
                        args: vec![rosc::OscType::Int(self.voice_chains[pos].group_id)],
                    }],
                    cleanup_time,
                )
                .map_err(|e| e.to_string())?;

            // Mark as released (keep in voice_chains for steal scoring)
            self.voice_chains[pos].release_state = Some((Instant::now(), release_time));
        }
        Ok(())
    }

    /// Release all active voices
    pub fn release_all_voices(&mut self) {
        if let Some(ref client) = self.client {
            for chain in self.voice_chains.drain(..) {
                let _ = client.free_node(chain.group_id);
            }
        }
    }

    /// Remove voices whose release envelope has fully expired.
    /// Called periodically from the audio thread to prevent unbounded growth.
    pub fn cleanup_expired_voices(&mut self) {
        let now = Instant::now();
        self.voice_chains.retain(|v| {
            if let Some((released_at, release_dur)) = v.release_state {
                // Keep if still within release + 1.5s safety margin
                now.duration_since(released_at).as_secs_f32() < release_dur + 1.5
            } else {
                true // active voice, keep
            }
        });
    }

    /// Find the best steal candidate for a given instrument.
    /// Returns the index of the voice with the lowest score (best target).
    ///
    /// Scoring:
    /// - Released voices: 0–999 (further into release = lower score)
    /// - Active voices: 1000+ (lower velocity = lower; older = lower tiebreaker)
    fn find_steal_candidate(&self, instrument_id: InstrumentId) -> Option<usize> {
        let now = Instant::now();

        self.voice_chains
            .iter()
            .enumerate()
            .filter(|(_, v)| v.instrument_id == instrument_id)
            .min_by(|(_, a), (_, b)| {
                let score_a = Self::steal_score(a, now);
                let score_b = Self::steal_score(b, now);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Compute a steal priority score for a voice. Lower = better steal target.
    fn steal_score(voice: &VoiceChain, now: Instant) -> f64 {
        if let Some((released_at, release_dur)) = voice.release_state {
            // Released voice: score 0–999
            // Further into release (closer to silent) = lower score
            let elapsed = now.duration_since(released_at).as_secs_f64();
            let progress = if release_dur > 0.0 {
                (elapsed / release_dur as f64).min(1.0)
            } else {
                1.0
            };
            // progress 1.0 (fully released) -> score 0, progress 0.0 (just released) -> score 999
            (1.0 - progress) * 999.0
        } else {
            // Active voice: score 1000+
            // Lower velocity = lower score (steal quieter voices first)
            let velocity_score = voice.velocity as f64 * 500.0; // 0–500
            // Older = lower score (tiebreaker)
            let age_secs = now.duration_since(voice.spawn_time).as_secs_f64();
            let age_score = 500.0 / (1.0 + age_secs); // older -> smaller value
            1000.0 + velocity_score + age_score
        }
    }

    /// Steal a voice if needed before spawning a new one.
    /// 1. Same-pitch retrigger: always steal matching pitch
    /// 2. If active voice count is at the limit, steal the best candidate
    /// Uses a brief anti-click fade (gate=0 then /n_free after 5ms) instead of hard cut.
    pub(crate) fn steal_voice_if_needed(
        &mut self,
        instrument_id: InstrumentId,
        pitch: u8,
        _velocity: f32,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        // 1. Same-pitch retrigger: steal any voice (active or released) with the same pitch
        if let Some(pos) = self.voice_chains.iter().position(|v| {
            v.instrument_id == instrument_id && v.pitch == pitch
        }) {
            let old = self.voice_chains.remove(pos);
            Self::anti_click_free(client.as_ref(), &old)?;
            // After removing the retrigger target, still check if we're at the limit
        }

        // 2. Count active (non-released) voices for this instrument
        let active_count = self
            .voice_chains
            .iter()
            .filter(|v| v.instrument_id == instrument_id && v.release_state.is_none())
            .count();

        if active_count >= MAX_VOICES_PER_INSTRUMENT {
            if let Some(pos) = self.find_steal_candidate(instrument_id) {
                let old = self.voice_chains.remove(pos);
                Self::anti_click_free(client.as_ref(), &old)?;
            }
        }

        Ok(())
    }

    /// Free a voice with a brief anti-click fade: send gate=0, then /n_free after 5ms.
    /// For already-released voices, skip gate=0 (already fading) and free immediately.
    fn anti_click_free(
        client: &dyn super::super::osc_client::OscClientLike,
        voice: &VoiceChain,
    ) -> Result<(), String> {
        if voice.release_state.is_some() {
            // Already releasing — just free immediately (deferred /n_free already scheduled,
            // but SC silently ignores double-frees)
            client.free_node(voice.group_id).map_err(|e| e.to_string())?;
        } else {
            // Active voice: send gate=0 for a brief fade, then free after 5ms
            let now = super::super::osc_client::osc_time_from_now(0.0);
            client
                .set_params_bundled(voice.midi_node_id, &[("gate", 0.0)], now)
                .map_err(|e| e.to_string())?;
            let free_time = super::super::osc_client::osc_time_from_now(0.005);
            client
                .send_bundle(
                    vec![rosc::OscMessage {
                        addr: "/n_free".to_string(),
                        args: vec![rosc::OscType::Int(voice.group_id)],
                    }],
                    free_time,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Play a one-shot drum sample routed through an instrument's signal chain
    pub fn play_drum_hit_to_instrument(
        &mut self,
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
        rate: f32,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let bufnum = *self.buffer_map.get(&buffer_id).ok_or("Buffer not loaded")?;
        let out_bus = self
            .bus_allocator
            .get_audio_bus(instrument_id, "source_out")
            .unwrap_or(0);

        let node_id = self.next_node_id;
        self.next_node_id += 1;

        client
            .send_message(
                "/s_new",
                vec![
                    rosc::OscType::String("imbolc_sampler_oneshot".to_string()),
                    rosc::OscType::Int(node_id),
                    rosc::OscType::Int(0), // addToHead
                    rosc::OscType::Int(GROUP_SOURCES),
                    rosc::OscType::String("bufnum".to_string()),
                    rosc::OscType::Int(bufnum),
                    rosc::OscType::String("amp".to_string()),
                    rosc::OscType::Float(amp),
                    rosc::OscType::String("sliceStart".to_string()),
                    rosc::OscType::Float(slice_start),
                    rosc::OscType::String("sliceEnd".to_string()),
                    rosc::OscType::Float(slice_end),
                    rosc::OscType::String("rate".to_string()),
                    rosc::OscType::Float(rate),
                    rosc::OscType::String("out".to_string()),
                    rosc::OscType::Int(out_bus), // Route to instrument's source bus
                ],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
