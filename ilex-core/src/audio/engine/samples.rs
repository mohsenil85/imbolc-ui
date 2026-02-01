use std::fs;
use std::path::Path;

use super::AudioEngine;
use crate::state::BufferId;

impl AudioEngine {
    pub fn load_synthdefs(&self, dir: &Path) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map_or(false, |e| e == "scsyndef") {
                let data = fs::read(&path).map_err(|e| e.to_string())?;
                client
                    .send_message("/d_recv", vec![rosc::OscType::Blob(data)])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Load a single .scsyndef file into the server
    pub fn load_synthdef_file(&self, path: &Path) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if path.extension().map_or(false, |e| e == "scsyndef") {
            let data = fs::read(path).map_err(|e| e.to_string())?;
            client
                .send_message("/d_recv", vec![rosc::OscType::Blob(data)])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // =========================================================================
    // Buffer Management (for Sampler)
    // =========================================================================

    /// Load a sample file into a SuperCollider buffer
    /// Returns the SC buffer number on success
    #[allow(dead_code)]
    pub fn load_sample(&mut self, buffer_id: BufferId, path: &str) -> Result<i32, String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        // Check if already loaded
        if let Some(&bufnum) = self.buffer_map.get(&buffer_id) {
            return Ok(bufnum);
        }

        let bufnum = self.next_bufnum;
        self.next_bufnum += 1;

        client.load_buffer(bufnum, path).map_err(|e| e.to_string())?;

        self.buffer_map.insert(buffer_id, bufnum);
        Ok(bufnum)
    }

    /// Free a sample buffer from SuperCollider
    #[allow(dead_code)]
    pub fn free_sample(&mut self, buffer_id: BufferId) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        if let Some(bufnum) = self.buffer_map.remove(&buffer_id) {
            client.free_buffer(bufnum).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get the SuperCollider buffer number for a loaded buffer
    #[allow(dead_code)]
    pub fn get_sc_bufnum(&self, buffer_id: BufferId) -> Option<i32> {
        self.buffer_map.get(&buffer_id).copied()
    }

    /// Check if a buffer is loaded
    #[allow(dead_code)]
    pub fn is_buffer_loaded(&self, buffer_id: BufferId) -> bool {
        self.buffer_map.contains_key(&buffer_id)
    }
}
