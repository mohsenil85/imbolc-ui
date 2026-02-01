/// Arpeggiator configuration, stored per-instrument.
#[derive(Debug, Clone)]
pub struct ArpeggiatorConfig {
    pub enabled: bool,
    pub direction: ArpDirection,
    pub rate: ArpRate,
    pub octaves: u8,     // 1-4
    pub gate: f32,       // 0.1-1.0 (note length as fraction of step)
}

impl Default for ArpeggiatorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            direction: ArpDirection::Up,
            rate: ArpRate::Eighth,
            octaves: 1,
            gate: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpDirection {
    Up,
    Down,
    UpDown,
    Random,
}

impl ArpDirection {
    pub fn name(&self) -> &'static str {
        match self {
            ArpDirection::Up => "Up",
            ArpDirection::Down => "Down",
            ArpDirection::UpDown => "Up/Down",
            ArpDirection::Random => "Random",
        }
    }

    pub fn next(&self) -> ArpDirection {
        match self {
            ArpDirection::Up => ArpDirection::Down,
            ArpDirection::Down => ArpDirection::UpDown,
            ArpDirection::UpDown => ArpDirection::Random,
            ArpDirection::Random => ArpDirection::Up,
        }
    }

    pub fn prev(&self) -> ArpDirection {
        match self {
            ArpDirection::Up => ArpDirection::Random,
            ArpDirection::Down => ArpDirection::Up,
            ArpDirection::UpDown => ArpDirection::Down,
            ArpDirection::Random => ArpDirection::UpDown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpRate {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl ArpRate {
    pub fn name(&self) -> &'static str {
        match self {
            ArpRate::Quarter => "1/4",
            ArpRate::Eighth => "1/8",
            ArpRate::Sixteenth => "1/16",
            ArpRate::ThirtySecond => "1/32",
        }
    }

    /// Steps per beat (quarter note)
    pub fn steps_per_beat(&self) -> f32 {
        match self {
            ArpRate::Quarter => 1.0,
            ArpRate::Eighth => 2.0,
            ArpRate::Sixteenth => 4.0,
            ArpRate::ThirtySecond => 8.0,
        }
    }

    pub fn next(&self) -> ArpRate {
        match self {
            ArpRate::Quarter => ArpRate::Eighth,
            ArpRate::Eighth => ArpRate::Sixteenth,
            ArpRate::Sixteenth => ArpRate::ThirtySecond,
            ArpRate::ThirtySecond => ArpRate::Quarter,
        }
    }

    pub fn prev(&self) -> ArpRate {
        match self {
            ArpRate::Quarter => ArpRate::ThirtySecond,
            ArpRate::Eighth => ArpRate::Quarter,
            ArpRate::Sixteenth => ArpRate::Eighth,
            ArpRate::ThirtySecond => ArpRate::Sixteenth,
        }
    }
}

/// Chord shape definitions — interval offsets from root in semitones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordShape {
    Major,
    Minor,
    Seventh,
    MinorSeventh,
    Sus2,
    Sus4,
    PowerChord,
    Octave,
}

impl ChordShape {
    /// Returns semitone offsets including the root (0).
    pub fn intervals(&self) -> &'static [i8] {
        match self {
            ChordShape::Major => &[0, 4, 7],
            ChordShape::Minor => &[0, 3, 7],
            ChordShape::Seventh => &[0, 4, 7, 10],
            ChordShape::MinorSeventh => &[0, 3, 7, 10],
            ChordShape::Sus2 => &[0, 2, 7],
            ChordShape::Sus4 => &[0, 5, 7],
            ChordShape::PowerChord => &[0, 7],
            ChordShape::Octave => &[0, 12],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ChordShape::Major => "Major",
            ChordShape::Minor => "Minor",
            ChordShape::Seventh => "7th",
            ChordShape::MinorSeventh => "m7",
            ChordShape::Sus2 => "sus2",
            ChordShape::Sus4 => "sus4",
            ChordShape::PowerChord => "Power",
            ChordShape::Octave => "Octave",
        }
    }

    pub fn next(&self) -> ChordShape {
        match self {
            ChordShape::Major => ChordShape::Minor,
            ChordShape::Minor => ChordShape::Seventh,
            ChordShape::Seventh => ChordShape::MinorSeventh,
            ChordShape::MinorSeventh => ChordShape::Sus2,
            ChordShape::Sus2 => ChordShape::Sus4,
            ChordShape::Sus4 => ChordShape::PowerChord,
            ChordShape::PowerChord => ChordShape::Octave,
            ChordShape::Octave => ChordShape::Major,
        }
    }

    pub fn prev(&self) -> ChordShape {
        match self {
            ChordShape::Major => ChordShape::Octave,
            ChordShape::Minor => ChordShape::Major,
            ChordShape::Seventh => ChordShape::Minor,
            ChordShape::MinorSeventh => ChordShape::Seventh,
            ChordShape::Sus2 => ChordShape::MinorSeventh,
            ChordShape::Sus4 => ChordShape::Sus2,
            ChordShape::PowerChord => ChordShape::Sus4,
            ChordShape::Octave => ChordShape::PowerChord,
        }
    }

    /// Expand a single MIDI pitch into chord pitches, clamped to valid MIDI range.
    pub fn expand(&self, root: u8) -> Vec<u8> {
        self.intervals()
            .iter()
            .filter_map(|&offset| {
                let pitch = root as i16 + offset as i16;
                if (0..=127).contains(&pitch) {
                    Some(pitch as u8)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Arpeggiator play state — runtime state tracked on the audio thread.
#[derive(Debug, Clone)]
pub struct ArpPlayState {
    pub held_notes: Vec<u8>,       // Currently held MIDI pitches (sorted)
    pub step_index: usize,         // Current position in the note sequence
    pub accumulator: f32,          // Fractional step accumulator
    pub ascending: bool,           // For UpDown direction tracking
    pub current_pitch: Option<u8>, // Currently sounding pitch (for release)
}

impl Default for ArpPlayState {
    fn default() -> Self {
        Self {
            held_notes: Vec::new(),
            step_index: 0,
            accumulator: 0.0,
            ascending: true,
            current_pitch: None,
        }
    }
}
