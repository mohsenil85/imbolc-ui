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

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Voice-steal: if at limit, free oldest by spawn_time
        let count = self.voice_chains.iter().filter(|v| v.instrument_id == instrument_id).count();
        if count >= MAX_VOICES_PER_INSTRUMENT {
            if let Some(pos) = self.voice_chains.iter()
                .enumerate()
                .filter(|(_, v)| v.instrument_id == instrument_id)
                .min_by_key(|(_, v)| v.spawn_time)
                .map(|(i, _)| i)
            {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

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
            group_id,
            midi_node_id,
            source_node: source_node_id,
            spawn_time: Instant::now(),
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

        let client = self.client.as_ref().ok_or("Not connected")?;

        // Voice-steal: if at limit, free oldest by spawn_time
        let count = self.voice_chains.iter().filter(|v| v.instrument_id == instrument_id).count();
        if count >= MAX_VOICES_PER_INSTRUMENT {
            if let Some(pos) = self.voice_chains.iter()
                .enumerate()
                .filter(|(_, v)| v.instrument_id == instrument_id)
                .min_by_key(|(_, v)| v.spawn_time)
                .map(|(i, _)| i)
            {
                let old = self.voice_chains.remove(pos);
                let _ = client.free_node(old.group_id);
            }
        }

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
            group_id,
            midi_node_id,
            source_node: sampler_node_id,
            spawn_time: Instant::now(),
        });

        Ok(())
    }

    /// Release a specific voice by instrument and pitch (note-off)
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

        if let Some(pos) = self
            .voice_chains
            .iter()
            .position(|v| v.instrument_id == instrument_id && v.pitch == pitch)
        {
            let chain = self.voice_chains.remove(pos);
            let time = super::super::osc_client::osc_time_from_now(offset_secs);
            client
                .set_params_bundled(chain.midi_node_id, &[("gate", 0.0)], time)
                .map_err(|e| e.to_string())?;
            // Schedule group free after envelope release completes (+1s margin)
            let release_time = state.instrument(instrument_id)
                .map(|s| s.amp_envelope.release)
                .unwrap_or(1.0);
            let cleanup_time = super::super::osc_client::osc_time_from_now(
                offset_secs + release_time as f64 + 1.0
            );
            client
                .send_bundle(
                    vec![rosc::OscMessage {
                        addr: "/n_free".to_string(),
                        args: vec![rosc::OscType::Int(chain.group_id)],
                    }],
                    cleanup_time,
                )
                .map_err(|e| e.to_string())?;
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

    /// Play a one-shot drum sample routed through an instrument's signal chain
    pub fn play_drum_hit_to_instrument(
        &mut self,
        buffer_id: BufferId,
        amp: f32,
        instrument_id: InstrumentId,
        slice_start: f32,
        slice_end: f32,
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
                    rosc::OscType::String("out".to_string()),
                    rosc::OscType::Int(out_bus), // Route to instrument's source bus
                ],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
