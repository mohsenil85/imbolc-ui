use super::AudioEngine;
use crate::state::{AutomationTarget, InstrumentState, SessionState};

impl AudioEngine {
    /// Apply an automation value to a target parameter
    /// This updates the appropriate synth node in real-time
    pub fn apply_automation(&self, target: &AutomationTarget, value: f32, state: &InstrumentState, session: &SessionState) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;

        match target {
            AutomationTarget::InstrumentLevel(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    let effective_level = value * session.master_level;
                    client.set_param(nodes.output, "level", effective_level)
                        .map_err(|e| e.to_string())?;
                }
            }
            AutomationTarget::InstrumentPan(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    client.set_param(nodes.output, "pan", value)
                        .map_err(|e| e.to_string())?;
                }
            }
            AutomationTarget::FilterCutoff(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(filter_node) = nodes.filter {
                        client.set_param(filter_node, "cutoff", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::FilterResonance(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(filter_node) = nodes.filter {
                        client.set_param(filter_node, "resonance", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::EffectParam(instrument_id, effect_id, param_idx) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(&effect_node) = nodes.effects.get(effect_id) {
                        if let Some(instrument) = state.instrument(*instrument_id) {
                            if let Some(effect) = instrument.effect_by_id(*effect_id) {
                                if let Some(param) = effect.params.get(*param_idx) {
                                    client.set_param(effect_node, &param.name, value)
                                        .map_err(|e| e.to_string())?;
                                }
                            }
                        }
                    }
                }
            }
            AutomationTarget::SampleRate(instrument_id) => {
                for voice in self.voice_allocator.chains() {
                    if voice.instrument_id == *instrument_id {
                        client.set_param(voice.source_node, "rate", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::SampleAmp(instrument_id) => {
                for voice in self.voice_allocator.chains() {
                    if voice.instrument_id == *instrument_id {
                        client.set_param(voice.source_node, "amp", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::LfoRate(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(lfo_node) = nodes.lfo {
                        client.set_param(lfo_node, "rate", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::LfoDepth(instrument_id) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(lfo_node) = nodes.lfo {
                        client.set_param(lfo_node, "depth", value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::EnvelopeAttack(instrument_id) => {
                // Update state — affects newly spawned voices only
                if let Some(inst) = state.instrument(*instrument_id) {
                    let _ = inst; // envelope params are read at voice spawn time
                }
            }
            AutomationTarget::EnvelopeDecay(instrument_id) => {
                if let Some(inst) = state.instrument(*instrument_id) {
                    let _ = inst;
                }
            }
            AutomationTarget::EnvelopeSustain(instrument_id) => {
                if let Some(inst) = state.instrument(*instrument_id) {
                    let _ = inst;
                }
            }
            AutomationTarget::EnvelopeRelease(instrument_id) => {
                if let Some(inst) = state.instrument(*instrument_id) {
                    let _ = inst;
                }
            }
            AutomationTarget::SendLevel(instrument_id, send_idx) => {
                if let Some(inst) = state.instrument(*instrument_id) {
                    if let Some(send) = inst.sends.get(*send_idx) {
                        if let Some(&node_id) = self.send_node_map.get(&(*instrument_id, send.bus_id)) {
                            client.set_param(node_id, "level", value)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            AutomationTarget::BusLevel(bus_id) => {
                if let Some(&node_id) = self.bus_node_map.get(bus_id) {
                    client.set_param(node_id, "level", value)
                        .map_err(|e| e.to_string())?;
                }
            }
            AutomationTarget::Bpm => {
                // Handled in playback.rs, not here
            }
            AutomationTarget::VstParam(instrument_id, param_index) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(source_node) = nodes.source {
                        use rosc::OscType;
                        client.send_unit_cmd(
                            source_node,
                            super::VST_UGEN_INDEX,
                            "/set",
                            vec![OscType::Int(*param_index as i32), OscType::Float(value)],
                        ).map_err(|e| e.to_string())?;
                    }
                }
            }
            AutomationTarget::EqBandParam(instrument_id, band, param) => {
                if let Some(nodes) = self.node_map.get(instrument_id) {
                    if let Some(eq_node) = nodes.eq {
                        let param_name = match param {
                            0 => format!("b{}_freq", band),
                            1 => format!("b{}_gain", band),
                            _ => format!("b{}_q", band),
                        };
                        let sc_value = if *param == 2 { 1.0 / value } else { value };
                        client.set_param(eq_node, &param_name, sc_value)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        Ok(())
    }
}
