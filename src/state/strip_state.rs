use serde::{Deserialize, Serialize};

use super::automation::AutomationState;
use super::midi_recording::MidiRecordingState;
use super::piano_roll::PianoRollState;
use super::strip::*;

use crate::ui::frame::SessionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixerSelection {
    Strip(usize),  // index into strips vec
    Bus(u8),       // 1-8
    Master,
}

impl Default for MixerSelection {
    fn default() -> Self {
        Self::Strip(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripState {
    pub strips: Vec<Strip>,
    pub selected: Option<usize>,
    pub next_id: StripId,
    pub buses: Vec<MixerBus>,
    pub master_level: f32,
    pub master_mute: bool,
    pub piano_roll: PianoRollState,
    pub mixer_selection: MixerSelection,
    pub automation: AutomationState,
    pub midi_recording: MidiRecordingState,
}

impl StripState {
    pub fn new() -> Self {
        let buses = (1..=MAX_BUSES as u8).map(MixerBus::new).collect();
        Self {
            strips: Vec::new(),
            selected: None,
            next_id: 0,
            buses,
            master_level: 1.0,
            master_mute: false,
            piano_roll: PianoRollState::new(),
            mixer_selection: MixerSelection::default(),
            automation: AutomationState::new(),
            midi_recording: MidiRecordingState::new(),
        }
    }

    pub fn add_strip(&mut self, source: OscType) -> StripId {
        let id = self.next_id;
        self.next_id += 1;
        let strip = Strip::new(id, source);

        // Auto-add piano roll track if strip has_track
        if strip.has_track {
            self.piano_roll.add_track(id);
        }

        self.strips.push(strip);

        if self.selected.is_none() {
            self.selected = Some(0);
        }

        id
    }

    pub fn remove_strip(&mut self, id: StripId) {
        if let Some(pos) = self.strips.iter().position(|s| s.id == id) {
            self.strips.remove(pos);
            self.piano_roll.remove_track(id);

            if let Some(sel) = self.selected {
                if sel >= self.strips.len() {
                    self.selected = if self.strips.is_empty() {
                        None
                    } else {
                        Some(self.strips.len() - 1)
                    };
                }
            }
        }
    }

    pub fn strip(&self, id: StripId) -> Option<&Strip> {
        self.strips.iter().find(|s| s.id == id)
    }

    pub fn strip_mut(&mut self, id: StripId) -> Option<&mut Strip> {
        self.strips.iter_mut().find(|s| s.id == id)
    }

    pub fn selected_strip(&self) -> Option<&Strip> {
        self.selected.and_then(|idx| self.strips.get(idx))
    }

    pub fn selected_strip_mut(&mut self) -> Option<&mut Strip> {
        self.selected.and_then(|idx| self.strips.get_mut(idx))
    }

    pub fn select_next(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(idx) if idx < self.strips.len() - 1 => Some(idx + 1),
            Some(idx) => Some(idx),
        };
    }

    pub fn select_prev(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(0) => Some(0),
            Some(idx) => Some(idx - 1),
        };
    }

    pub fn bus(&self, id: u8) -> Option<&MixerBus> {
        self.buses.get((id - 1) as usize)
    }

    pub fn bus_mut(&mut self, id: u8) -> Option<&mut MixerBus> {
        self.buses.get_mut((id - 1) as usize)
    }

    /// Check if any strip is soloed
    pub fn any_strip_solo(&self) -> bool {
        self.strips.iter().any(|s| s.solo)
    }

    /// Check if any bus is soloed
    pub fn any_bus_solo(&self) -> bool {
        self.buses.iter().any(|b| b.solo)
    }

    /// Compute effective mute for a strip, considering solo state
    pub fn effective_strip_mute(&self, strip: &Strip) -> bool {
        if self.any_strip_solo() {
            !strip.solo
        } else {
            strip.mute || self.master_mute
        }
    }

    /// Compute effective mute for a bus, considering solo state
    pub fn effective_bus_mute(&self, bus: &MixerBus) -> bool {
        if self.any_bus_solo() {
            !bus.solo
        } else {
            bus.mute
        }
    }

    /// Collect mixer updates for all strips (strip_id, level, mute)
    pub fn collect_strip_updates(&self) -> Vec<(StripId, f32, bool)> {
        self.strips
            .iter()
            .map(|s| (s.id, s.level * self.master_level, self.effective_strip_mute(s)))
            .collect()
    }

    /// Move mixer selection left/right
    pub fn mixer_move(&mut self, delta: i8) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(idx) => {
                let new_idx = (idx as i32 + delta as i32).clamp(0, self.strips.len().saturating_sub(1) as i32) as usize;
                MixerSelection::Strip(new_idx)
            }
            MixerSelection::Bus(id) => {
                let new_id = (id as i8 + delta).clamp(1, MAX_BUSES as i8) as u8;
                MixerSelection::Bus(new_id)
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Jump to first (1) or last (-1) in current section
    pub fn mixer_jump(&mut self, direction: i8) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(_) => {
                if direction > 0 {
                    MixerSelection::Strip(0)
                } else {
                    MixerSelection::Strip(self.strips.len().saturating_sub(1))
                }
            }
            MixerSelection::Bus(_) => {
                if direction > 0 {
                    MixerSelection::Bus(1)
                } else {
                    MixerSelection::Bus(MAX_BUSES as u8)
                }
            }
            MixerSelection::Master => MixerSelection::Master,
        };
    }

    /// Cycle between strip/bus/master sections
    pub fn mixer_cycle_section(&mut self) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Strip(_) => MixerSelection::Bus(1),
            MixerSelection::Bus(_) => MixerSelection::Master,
            MixerSelection::Master => MixerSelection::Strip(0),
        };
    }

    /// Cycle output target for the selected strip
    pub fn mixer_cycle_output(&mut self) {
        if let MixerSelection::Strip(idx) = self.mixer_selection {
            if let Some(strip) = self.strips.get_mut(idx) {
                strip.output_target = match strip.output_target {
                    OutputTarget::Master => OutputTarget::Bus(1),
                    OutputTarget::Bus(n) if n < MAX_BUSES as u8 => OutputTarget::Bus(n + 1),
                    OutputTarget::Bus(_) => OutputTarget::Master,
                };
            }
        }
    }

    /// Cycle output target backwards for the selected strip
    pub fn mixer_cycle_output_reverse(&mut self) {
        if let MixerSelection::Strip(idx) = self.mixer_selection {
            if let Some(strip) = self.strips.get_mut(idx) {
                strip.output_target = match strip.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }

    /// Strips that have tracks (for piano roll)
    pub fn strips_with_tracks(&self) -> Vec<&Strip> {
        self.strips.iter().filter(|s| s.has_track).collect()
    }
}

impl Default for StripState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_state_creation() {
        let state = StripState::new();
        assert_eq!(state.strips.len(), 0);
        assert_eq!(state.selected, None);
        assert_eq!(state.buses.len(), MAX_BUSES);
    }

    #[test]
    fn test_add_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
        assert_eq!(state.strips[1].id, id2);
        assert_eq!(state.selected, Some(0));
        // Piano roll tracks auto-created
        assert_eq!(state.piano_roll.track_order.len(), 2);
    }

    #[test]
    fn test_remove_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);
        let _id3 = state.add_strip(OscType::Sqr);

        state.remove_strip(id2);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
        assert_eq!(state.piano_roll.track_order.len(), 2);
    }

    #[test]
    fn test_remove_last_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        state.selected = Some(1);
        state.remove_strip(id2);

        assert_eq!(state.selected, Some(0));
        assert_eq!(state.strips[0].id, id1);
    }

    #[test]
    fn test_remove_all_strips() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);

        state.remove_strip(id1);
        assert_eq!(state.selected, None);
        assert!(state.strips.is_empty());
    }

    #[test]
    fn test_select_navigation() {
        let mut state = StripState::new();
        state.add_strip(OscType::Saw);
        state.add_strip(OscType::Sin);
        state.add_strip(OscType::Sqr);

        assert_eq!(state.selected, Some(0));
        state.select_next();
        assert_eq!(state.selected, Some(1));
        state.select_next();
        assert_eq!(state.selected, Some(2));
        state.select_next();
        assert_eq!(state.selected, Some(2)); // stay at end
        state.select_prev();
        assert_eq!(state.selected, Some(1));
        state.select_prev();
        assert_eq!(state.selected, Some(0));
        state.select_prev();
        assert_eq!(state.selected, Some(0)); // stay at start
    }

    #[test]
    fn test_mixer_selection() {
        let mut state = StripState::new();
        state.add_strip(OscType::Saw);
        state.add_strip(OscType::Sin);

        state.mixer_selection = MixerSelection::Strip(0);
        state.mixer_move(1);
        assert_eq!(state.mixer_selection, MixerSelection::Strip(1));

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Bus(1));

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Master);

        state.mixer_cycle_section();
        assert_eq!(state.mixer_selection, MixerSelection::Strip(0));
    }

    #[test]
    fn test_save_and_load() {
        use tempfile::tempdir;

        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let _id2 = state.add_strip(OscType::Sin);

        // Add a filter to first strip
        if let Some(strip) = state.strip_mut(id1) {
            strip.filter = Some(FilterConfig::new(FilterType::Lpf));
            strip.effects.push(EffectSlot::new(EffectType::Reverb));
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");
        assert_eq!(loaded.strips.len(), 2);
        assert_eq!(loaded.strips[0].source, OscType::Saw);
        assert_eq!(loaded.strips[1].source, OscType::Sin);
        assert!(loaded.strips[0].filter.is_some());
        assert_eq!(loaded.strips[0].effects.len(), 1);
        assert_eq!(loaded.next_id, 2);
    }

    #[test]
    fn test_modulation_persistence() {
        use tempfile::tempdir;

        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);

        // Add a filter with modulation sources
        if let Some(strip) = state.strip_mut(id1) {
            let mut filter = FilterConfig::new(FilterType::Lpf);
            filter.cutoff.mod_source = Some(ModSource::Lfo(LfoConfig {
                enabled: true,
                rate: 2.5,
                depth: 0.8,
                shape: LfoShape::Sine,
                target: LfoTarget::FilterCutoff,
            }));
            filter.resonance.mod_source = Some(ModSource::Envelope(EnvConfig {
                attack: 0.05,
                decay: 0.2,
                sustain: 0.6,
                release: 0.4,
            }));
            strip.filter = Some(filter);
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_mod.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");
        let filter = loaded.strips[0].filter.as_ref().expect("Filter should exist");

        // Verify LFO on cutoff
        match &filter.cutoff.mod_source {
            Some(ModSource::Lfo(lfo)) => {
                assert!((lfo.rate - 2.5).abs() < 0.01, "LFO rate mismatch");
                assert!((lfo.depth - 0.8).abs() < 0.01, "LFO depth mismatch");
            }
            _ => panic!("Expected LFO mod source on cutoff"),
        }

        // Verify envelope on resonance
        match &filter.resonance.mod_source {
            Some(ModSource::Envelope(env)) => {
                assert!((env.attack - 0.05).abs() < 0.01, "Envelope attack mismatch");
                assert!((env.decay - 0.2).abs() < 0.01, "Envelope decay mismatch");
                assert!((env.sustain - 0.6).abs() < 0.01, "Envelope sustain mismatch");
                assert!((env.release - 0.4).abs() < 0.01, "Envelope release mismatch");
            }
            _ => panic!("Expected Envelope mod source on resonance"),
        }
    }

    #[test]
    fn test_strip_param_modulation_persistence() {
        use tempfile::tempdir;

        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        // Add a filter with StripParam modulation source
        if let Some(strip) = state.strip_mut(id1) {
            let mut filter = FilterConfig::new(FilterType::Lpf);
            filter.cutoff.mod_source = Some(ModSource::StripParam(id2, "amp".to_string()));
            strip.filter = Some(filter);
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_strip_param_mod.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");
        let filter = loaded.strips[0].filter.as_ref().expect("Filter should exist");

        match &filter.cutoff.mod_source {
            Some(ModSource::StripParam(src_id, param_name)) => {
                assert_eq!(*src_id, id2, "Source strip ID mismatch");
                assert_eq!(param_name, "amp", "Param name mismatch");
            }
            _ => panic!("Expected StripParam mod source on cutoff"),
        }
    }

    #[test]
    fn test_sampler_config_persistence() {
        use tempfile::tempdir;
        use super::super::sampler::Slice;

        let mut state = StripState::new();
        let id = state.add_strip(OscType::Sampler);

        // Configure the sampler
        if let Some(strip) = state.strip_mut(id) {
            if let Some(ref mut config) = strip.sampler_config {
                config.buffer_id = Some(42);
                config.loop_mode = true;
                config.pitch_tracking = false;
                // Add a custom slice
                config.slices.clear();
                config.slices.push(Slice {
                    id: 0,
                    start: 0.25,
                    end: 0.75,
                    name: "Test Slice".to_string(),
                    root_note: 72,
                });
            }
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_sampler.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");
        let config = loaded.strips[0].sampler_config.as_ref().expect("Sampler config should exist");

        assert_eq!(config.buffer_id, Some(42));
        assert!(config.loop_mode);
        assert!(!config.pitch_tracking);
        assert_eq!(config.slices.len(), 1);
        assert!((config.slices[0].start - 0.25).abs() < 0.01);
        assert!((config.slices[0].end - 0.75).abs() < 0.01);
        assert_eq!(config.slices[0].name, "Test Slice");
        assert_eq!(config.slices[0].root_note, 72);
    }

    #[test]
    fn test_automation_persistence() {
        use tempfile::tempdir;
        use super::super::automation::{AutomationTarget, CurveType};

        let mut state = StripState::new();
        let id = state.add_strip(OscType::Saw);

        // Add automation lanes
        let lane_id = state.automation.add_lane(AutomationTarget::FilterCutoff(id));
        if let Some(lane) = state.automation.lane_mut(lane_id) {
            lane.add_point(0, 0.0);
            lane.add_point(480, 0.5);
            lane.add_point(960, 1.0);
            // Set the curve type on the first point
            if let Some(point) = lane.points.get_mut(0) {
                point.curve = CurveType::Exponential;
            }
        }

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_automation.tuidaw");
        let session = SessionState::default();
        state.save(&path, &session).expect("Failed to save");

        let (loaded, _) = StripState::load(&path).expect("Failed to load");

        assert_eq!(loaded.automation.lanes.len(), 1);
        let lane = &loaded.automation.lanes[0];
        assert_eq!(lane.target, AutomationTarget::FilterCutoff(id));
        assert_eq!(lane.points.len(), 3);
        assert_eq!(lane.points[0].tick, 0);
        assert!((lane.points[0].value - 0.0).abs() < 0.01);
        assert_eq!(lane.points[0].curve, CurveType::Exponential);
        assert_eq!(lane.points[1].tick, 480);
        assert!((lane.points[1].value - 0.5).abs() < 0.01);
        assert_eq!(lane.points[2].tick, 960);
        assert!((lane.points[2].value - 1.0).abs() < 0.01);
    }
}
