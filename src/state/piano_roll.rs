use std::collections::HashMap;

use super::module::ModuleId;

#[derive(Debug, Clone)]
pub struct Note {
    pub tick: u32,
    pub duration: u32,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub module_id: ModuleId,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone)]
pub struct PianoRollState {
    pub tracks: HashMap<ModuleId, Track>,
    pub track_order: Vec<ModuleId>,
    pub bpm: f32,
    pub time_signature: (u8, u8),
    pub playing: bool,
    pub looping: bool,
    pub loop_start: u32,
    pub loop_end: u32,
    pub playhead: u32,
    pub ticks_per_beat: u32,
}

impl PianoRollState {
    pub fn new() -> Self {
        Self {
            tracks: HashMap::new(),
            track_order: Vec::new(),
            bpm: 120.0,
            time_signature: (4, 4),
            playing: false,
            looping: false,
            loop_start: 0,
            loop_end: 480 * 4, // 4 beats
            playhead: 0,
            ticks_per_beat: 480,
        }
    }

    pub fn add_track(&mut self, module_id: ModuleId) {
        if !self.tracks.contains_key(&module_id) {
            self.tracks.insert(
                module_id,
                Track {
                    module_id,
                    notes: Vec::new(),
                },
            );
            self.track_order.push(module_id);
        }
    }

    pub fn remove_track(&mut self, module_id: ModuleId) {
        self.tracks.remove(&module_id);
        self.track_order.retain(|&id| id != module_id);
    }

    /// Get the track at the given index in track_order
    pub fn track_at(&self, index: usize) -> Option<&Track> {
        self.track_order
            .get(index)
            .and_then(|id| self.tracks.get(id))
    }

    /// Get a mutable track at the given index
    pub fn track_at_mut(&mut self, index: usize) -> Option<&mut Track> {
        let id = self.track_order.get(index).copied();
        id.and_then(move |id| self.tracks.get_mut(&id))
    }

    /// Toggle a note at the given position. If a note exists there, remove it; otherwise add one.
    pub fn toggle_note(&mut self, track_index: usize, pitch: u8, tick: u32, duration: u32, velocity: u8) {
        if let Some(track) = self.track_at_mut(track_index) {
            // Check if a note exists at this pitch/tick
            if let Some(pos) = track.notes.iter().position(|n| n.pitch == pitch && n.tick == tick) {
                track.notes.remove(pos);
            } else {
                track.notes.push(Note {
                    tick,
                    duration,
                    pitch,
                    velocity,
                });
            }
        }
    }

    /// Find a note at the given pitch and tick (exact match on tick start)
    pub fn find_note(&self, track_index: usize, pitch: u8, tick: u32) -> Option<&Note> {
        self.track_at(track_index)
            .and_then(|track| track.notes.iter().find(|n| n.pitch == pitch && n.tick == tick))
    }

    /// Find notes that start within a tick range (for playback)
    pub fn notes_in_range(&self, track_index: usize, start_tick: u32, end_tick: u32) -> Vec<&Note> {
        if let Some(track) = self.track_at(track_index) {
            track
                .notes
                .iter()
                .filter(|n| n.tick >= start_tick && n.tick < end_tick)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Advance playhead by a number of ticks, handling loop wrapping
    pub fn advance(&mut self, ticks: u32) {
        if !self.playing {
            return;
        }
        self.playhead += ticks;
        if self.looping && self.playhead >= self.loop_end {
            self.playhead = self.loop_start + (self.playhead - self.loop_end);
        }
    }

    /// Convert a beat number to ticks
    pub fn beat_to_tick(&self, beat: u32) -> u32 {
        beat * self.ticks_per_beat
    }

    /// Convert ticks to beat number (float)
    pub fn tick_to_beat(&self, tick: u32) -> f32 {
        tick as f32 / self.ticks_per_beat as f32
    }

    /// Total ticks per bar
    pub fn ticks_per_bar(&self) -> u32 {
        self.ticks_per_beat * self.time_signature.0 as u32
    }
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self::new()
    }
}
