use super::VST_UGEN_INDEX;
use super::AudioEngine;
use crate::state::InstrumentId;

impl AudioEngine {
    /// Send MIDI note-on to a VSTi persistent source node
    pub(super) fn send_vsti_note_on(&self, instrument_id: InstrumentId, pitch: u8, velocity: f32) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        if let Some(nodes) = self.node_map.get(&instrument_id) {
            if let Some(source_node) = nodes.source {
                let vel = (velocity * 127.0).round().min(127.0) as u8;
                // MIDI note-on: status 0x90, note, velocity as raw bytes
                let midi_msg: Vec<u8> = vec![0x90, pitch, vel];
                client.send_unit_cmd(
                    source_node,
                    VST_UGEN_INDEX,
                    "/midi_msg",
                    vec![rosc::OscType::Blob(midi_msg)],
                ).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Send MIDI note-off to a VSTi persistent source node
    pub(super) fn send_vsti_note_off(&self, instrument_id: InstrumentId, pitch: u8) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        if let Some(nodes) = self.node_map.get(&instrument_id) {
            if let Some(source_node) = nodes.source {
                // MIDI note-off: status 0x80, note, velocity 0
                let midi_msg: Vec<u8> = vec![0x80, pitch, 0];
                client.send_unit_cmd(
                    source_node,
                    VST_UGEN_INDEX,
                    "/midi_msg",
                    vec![rosc::OscType::Blob(midi_msg)],
                ).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
