#![allow(dead_code)]

use serde::{Serialize, Deserialize};

/// Musical key (pitch class)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    C, Cs, D, Ds, E, F, Fs, G, Gs, A, As, B,
}

impl Key {
    pub const ALL: [Key; 12] = [
        Key::C, Key::Cs, Key::D, Key::Ds, Key::E, Key::F,
        Key::Fs, Key::G, Key::Gs, Key::A, Key::As, Key::B,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Key::C => "C", Key::Cs => "C#", Key::D => "D", Key::Ds => "D#",
            Key::E => "E", Key::F => "F", Key::Fs => "F#", Key::G => "G",
            Key::Gs => "G#", Key::A => "A", Key::As => "A#", Key::B => "B",
        }
    }

    /// MIDI note number for this key in octave 0
    pub fn semitone(&self) -> i32 {
        match self {
            Key::C => 0, Key::Cs => 1, Key::D => 2, Key::Ds => 3,
            Key::E => 4, Key::F => 5, Key::Fs => 6, Key::G => 7,
            Key::Gs => 8, Key::A => 9, Key::As => 10, Key::B => 11,
        }
    }
}

/// Scale definition as intervals from root
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scale {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    Pentatonic,
    Blues,
    Chromatic,
}

impl Scale {
    pub const ALL: [Scale; 11] = [
        Scale::Major, Scale::Minor, Scale::Dorian, Scale::Phrygian,
        Scale::Lydian, Scale::Mixolydian, Scale::Aeolian, Scale::Locrian,
        Scale::Pentatonic, Scale::Blues, Scale::Chromatic,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Scale::Major => "Major",
            Scale::Minor => "Minor",
            Scale::Dorian => "Dorian",
            Scale::Phrygian => "Phrygian",
            Scale::Lydian => "Lydian",
            Scale::Mixolydian => "Mixolydian",
            Scale::Aeolian => "Aeolian",
            Scale::Locrian => "Locrian",
            Scale::Pentatonic => "Pentatonic",
            Scale::Blues => "Blues",
            Scale::Chromatic => "Chromatic",
        }
    }

    /// Semitone intervals from root for this scale
    pub fn intervals(&self) -> &'static [i32] {
        match self {
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Scale::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Scale::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Scale::Aeolian => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Scale::Pentatonic => &[0, 2, 4, 7, 9],
            Scale::Blues => &[0, 3, 5, 6, 7, 10],
            Scale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }
}

/// Snap a frequency to the nearest scale degree
/// `tuning_a4` is the reference frequency for A4 (default 440.0)
pub fn snap_freq_to_scale(freq: f32, key: Key, scale: Scale, tuning_a4: f32) -> f32 {
    if freq <= 0.0 {
        return freq;
    }

    // Convert freq to MIDI note number (continuous)
    let midi_note = 69.0 + 12.0 * (freq / tuning_a4).ln() / (2.0_f32).ln();

    // Find nearest scale degree
    let intervals = scale.intervals();
    let root_semitone = key.semitone();

    let rounded_note = midi_note.round() as i32;

    // Search nearby notes for closest scale degree
    let mut best_note = rounded_note;
    let mut best_dist = i32::MAX;

    for offset in -2..=2 {
        let candidate = rounded_note + offset;
        let relative = ((candidate - root_semitone) % 12 + 12) % 12;
        if intervals.contains(&relative) {
            let dist = (candidate as f32 - midi_note).abs() as i32;
            if dist < best_dist {
                best_dist = dist;
                best_note = candidate;
            }
        }
    }

    // Convert back to frequency
    tuning_a4 * (2.0_f32).powf((best_note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_freq_negative_or_zero_returns_input() {
        assert!((snap_freq_to_scale(-1.0, Key::C, Scale::Major, 440.0) + 1.0).abs() < 0.0001);
        assert!(snap_freq_to_scale(0.0, Key::C, Scale::Major, 440.0).abs() < 0.0001);
    }

    #[test]
    fn snap_freq_c_major_keeps_scale_degree() {
        let snapped = snap_freq_to_scale(440.0, Key::C, Scale::Major, 440.0);
        assert!((snapped - 440.0).abs() < 0.01);
    }

    #[test]
    fn snap_freq_c_major_prefers_nearest_scale_tone() {
        // C#4 is closer to C4 than D4, so expect snap to C4 in C major.
        let c_sharp_4 = 277.18;
        let snapped = snap_freq_to_scale(c_sharp_4, Key::C, Scale::Major, 440.0);
        let c4 = 261.63;
        let d4 = 293.66;
        assert!((snapped - c4).abs() < (snapped - d4).abs());
    }
}
