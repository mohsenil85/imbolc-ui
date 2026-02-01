use super::sampler::{BufferId, Slice, SliceId};

pub const NUM_PADS: usize = 12;
#[allow(dead_code)]
pub const MAX_STEPS: usize = 64;
pub const DEFAULT_STEPS: usize = 16;
pub const NUM_PATTERNS: usize = 4;

#[derive(Debug, Clone)]
pub struct ChopperState {
    pub buffer_id: Option<BufferId>,
    pub path: Option<String>,
    pub name: String,
    pub slices: Vec<Slice>,
    pub selected_slice: usize,
    pub next_slice_id: SliceId,
    pub waveform_peaks: Vec<f32>,
    pub duration_secs: f32,
}

#[derive(Debug, Clone)]
pub struct DrumStep {
    pub active: bool,
    pub velocity: u8, // 1-127, default 100
}

impl Default for DrumStep {
    fn default() -> Self {
        Self {
            active: false,
            velocity: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrumPad {
    pub buffer_id: Option<BufferId>,
    pub path: Option<String>,
    pub name: String,
    pub level: f32, // 0.0-1.0, default 0.8
    pub slice_start: f32, // 0.0-1.0, default 0.0
    pub slice_end: f32,   // 0.0-1.0, default 1.0
}

impl Default for DrumPad {
    fn default() -> Self {
        Self {
            buffer_id: None,
            path: None,
            name: String::new(),
            level: 0.8,
            slice_start: 0.0,
            slice_end: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrumPattern {
    pub steps: Vec<Vec<DrumStep>>, // [NUM_PADS][length]
    pub length: usize,
}

impl DrumPattern {
    pub fn new(length: usize) -> Self {
        Self {
            steps: (0..NUM_PADS)
                .map(|_| (0..length).map(|_| DrumStep::default()).collect())
                .collect(),
            length,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrumSequencerState {
    pub pads: Vec<DrumPad>,
    pub patterns: Vec<DrumPattern>,
    pub current_pattern: usize,
    pub playing: bool,
    pub current_step: usize,
    pub next_buffer_id: BufferId,
    pub step_accumulator: f32,
    pub last_played_step: Option<usize>,
    pub chopper: Option<ChopperState>,
}

impl DrumSequencerState {
    pub fn new() -> Self {
        Self {
            pads: (0..NUM_PADS).map(|_| DrumPad::default()).collect(),
            patterns: (0..NUM_PATTERNS)
                .map(|_| DrumPattern::new(DEFAULT_STEPS))
                .collect(),
            current_pattern: 0,
            playing: false,
            current_step: 0,
            next_buffer_id: 10000,
            step_accumulator: 0.0,
            last_played_step: None,
            chopper: None,
        }
    }

    pub fn pattern(&self) -> &DrumPattern {
        &self.patterns[self.current_pattern]
    }

    pub fn pattern_mut(&mut self) -> &mut DrumPattern {
        &mut self.patterns[self.current_pattern]
    }
}

impl Default for DrumSequencerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drum_sequencer_new() {
        let seq = DrumSequencerState::new();
        assert_eq!(seq.pads.len(), NUM_PADS);
        assert_eq!(seq.patterns.len(), NUM_PATTERNS);
        assert_eq!(seq.pattern().length, DEFAULT_STEPS);
        assert!(!seq.playing);
    }

    #[test]
    fn test_drum_pattern_new() {
        let pattern = DrumPattern::new(16);
        assert_eq!(pattern.steps.len(), NUM_PADS);
        assert_eq!(pattern.steps[0].len(), 16);
        assert!(!pattern.steps[0][0].active);
    }

    #[test]
    fn test_toggle_step() {
        let mut seq = DrumSequencerState::new();
        seq.pattern_mut().steps[0][0].active = true;
        assert!(seq.pattern().steps[0][0].active);
        seq.pattern_mut().steps[0][0].active = false;
        assert!(!seq.pattern().steps[0][0].active);
    }

    #[test]
    fn test_pattern_switching() {
        let mut seq = DrumSequencerState::new();
        seq.pattern_mut().steps[0][0].active = true;
        seq.current_pattern = 1;
        assert!(!seq.pattern().steps[0][0].active);
        seq.current_pattern = 0;
        assert!(seq.pattern().steps[0][0].active);
    }
}
