use super::drum_sequencer::DrumStep;

/// A note stored with position relative to the selection anchor.
/// anchor = (min_tick of selected notes, min_pitch of selected notes)
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardNote {
    pub tick_offset: u32,    // tick - anchor_tick
    pub pitch_offset: i16,   // pitch as i16 - anchor_pitch as i16
    pub duration: u32,
    pub velocity: u8,
    pub probability: f32,
}

/// Clipboard contents — one variant per context
#[derive(Debug, Clone)]
pub enum ClipboardContents {
    /// Piano roll notes with relative positions
    PianoRollNotes(Vec<ClipboardNote>),
    /// Drum sequencer steps: Vec<(pad_index, step_offset, DrumStep)>
    DrumSteps {
        steps: Vec<(usize, usize, DrumStep)>, // (pad_idx, step_offset, step_data)
    },
    /// Automation points: Vec<(tick_offset, value)>
    AutomationPoints {
        points: Vec<(u32, f32)>, // (tick_offset, value)
    },
}

/// App-wide clipboard (lives in AppState)
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    pub contents: Option<ClipboardContents>,
}
