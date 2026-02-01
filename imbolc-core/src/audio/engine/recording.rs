use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{AudioEngine, GROUP_RECORD};
use crate::audio::osc_client::osc_time_immediate;

/// State for an active disk recording session
pub(super) struct RecordingState {
    pub bufnum: i32,
    pub node_id: i32,
    pub path: PathBuf,
    pub started_at: Instant,
}

impl AudioEngine {
    /// Buffer number reserved for disk recording (well above sampler range)
    const RECORD_BUFNUM: i32 = 900;

    /// Start recording audio from the given bus to a WAV file.
    pub fn start_recording(&mut self, bus: i32, path: &Path) -> Result<(), String> {
        if self.recording.is_some() {
            return Err("Already recording".to_string());
        }
        let client = self.client.as_ref().ok_or("Not connected")?;

        let path_str = path.to_string_lossy().to_string();
        let node_id = self.next_node_id;
        self.next_node_id += 1;

        // Send all three commands as a single bundle for atomic execution:
        // 1. Allocate ring buffer  2. Open for disk write  3. Create DiskOut synth
        let messages = vec![
            rosc::OscMessage {
                addr: "/b_alloc".to_string(),
                args: vec![
                    rosc::OscType::Int(Self::RECORD_BUFNUM),
                    rosc::OscType::Int(131072),
                    rosc::OscType::Int(2),
                ],
            },
            rosc::OscMessage {
                addr: "/b_write".to_string(),
                args: vec![
                    rosc::OscType::Int(Self::RECORD_BUFNUM),
                    rosc::OscType::String(path_str),
                    rosc::OscType::String("wav".to_string()),
                    rosc::OscType::String("float".to_string()),
                    rosc::OscType::Int(0),
                    rosc::OscType::Int(0),
                    rosc::OscType::Int(1),
                ],
            },
            rosc::OscMessage {
                addr: "/s_new".to_string(),
                args: vec![
                    rosc::OscType::String("imbolc_disk_record".to_string()),
                    rosc::OscType::Int(node_id),
                    rosc::OscType::Int(1), // addToTail
                    rosc::OscType::Int(GROUP_RECORD),
                    rosc::OscType::String("bufnum".to_string()),
                    rosc::OscType::Float(Self::RECORD_BUFNUM as f32),
                    rosc::OscType::String("in".to_string()),
                    rosc::OscType::Float(bus as f32),
                ],
            },
        ];
        client.send_bundle(messages, osc_time_immediate())
            .map_err(|e| e.to_string())?;

        self.recording = Some(RecordingState {
            bufnum: Self::RECORD_BUFNUM,
            node_id,
            path: path.to_path_buf(),
            started_at: Instant::now(),
        });

        Ok(())
    }

    /// Stop the active recording and return the path of the recorded file.
    /// The buffer is not freed immediately — call `poll_pending_buffer_free()` in the
    /// main loop to free it after SuperCollider has flushed the file to disk.
    pub fn stop_recording(&mut self) -> Option<PathBuf> {
        let rec = self.recording.take()?;
        if let Some(ref client) = self.client {
            // Bundle node free + buffer close for atomic execution
            let messages = vec![
                rosc::OscMessage {
                    addr: "/n_free".to_string(),
                    args: vec![rosc::OscType::Int(rec.node_id)],
                },
                rosc::OscMessage {
                    addr: "/b_close".to_string(),
                    args: vec![rosc::OscType::Int(rec.bufnum)],
                },
            ];
            let _ = client.send_bundle(messages, osc_time_immediate());
            // Defer buffer free to give scsynth time to flush the file
            self.pending_buffer_free = Some((rec.bufnum, Instant::now()));
        }
        Some(rec.path)
    }

    /// Free any pending recording buffer after a delay.
    /// Returns true if a buffer was freed this call.
    pub fn poll_pending_buffer_free(&mut self) -> bool {
        if let Some((bufnum, when)) = self.pending_buffer_free {
            if when.elapsed() >= Duration::from_millis(500) {
                if let Some(ref client) = self.client {
                    let _ = client.free_buffer(bufnum);
                }
                self.pending_buffer_free = None;
                return true;
            }
        }
        false
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn recording_elapsed(&self) -> Option<Duration> {
        self.recording.as_ref().map(|r| r.started_at.elapsed())
    }

    pub fn recording_path(&self) -> Option<&Path> {
        self.recording.as_ref().map(|r| r.path.as_path())
    }
}
