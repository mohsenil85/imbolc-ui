/// Piano keyboard layout starting note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PianoLayout {
    C,
    A,
}

/// Shared piano keyboard state and key-to-pitch mapping.
///
/// Used by InstrumentPane, PianoRollPane, and InstrumentEditPane.
///
/// Available methods:
/// - `activate()` / `deactivate()` / `is_active()` — toggle piano mode
/// - `key_to_pitch(char) -> Option<u8>` — map keyboard char to MIDI pitch
/// - `handle_escape() -> bool` — cycle C→A→off, returns true if deactivated
/// - `octave_up()` / `octave_down()` — change octave (returns new octave)
/// - `octave()` — current octave
/// - `status_label() -> String` — e.g. "PIANO C4"
///
/// No `set_layout()` or `set_octave()` methods exist.
pub struct PianoKeyboard {
    active: bool,
    octave: i8,
    layout: PianoLayout,
}

impl PianoKeyboard {
    pub fn new() -> Self {
        Self {
            active: false,
            octave: 4,
            layout: PianoLayout::C,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.layout = PianoLayout::C;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn octave(&self) -> i8 {
        self.octave
    }

    /// Cycle layout C→A→off. Returns true if piano mode was deactivated.
    pub fn handle_escape(&mut self) -> bool {
        match self.layout {
            PianoLayout::C => {
                self.layout = PianoLayout::A;
                false
            }
            PianoLayout::A => {
                self.active = false;
                true
            }
        }
    }

    /// Decrease octave. Returns true if changed.
    pub fn octave_down(&mut self) -> bool {
        if self.octave > -1 {
            self.octave -= 1;
            true
        } else {
            false
        }
    }

    /// Increase octave. Returns true if changed.
    pub fn octave_up(&mut self) -> bool {
        if self.octave < 9 {
            self.octave += 1;
            true
        } else {
            false
        }
    }

    /// Status label for rendering, e.g. "PIANO C4".
    pub fn status_label(&self) -> String {
        let layout_char = match self.layout {
            PianoLayout::C => 'C',
            PianoLayout::A => 'A',
        };
        format!(" PIANO {}{} ", layout_char, self.octave)
    }

    /// Convert a keyboard character to a MIDI pitch using current octave and layout.
    pub fn key_to_pitch(&self, key: char) -> Option<u8> {
        let offset = match self.layout {
            PianoLayout::C => Self::key_to_offset_c(key),
            PianoLayout::A => Self::key_to_offset_a(key),
        };
        offset.map(|off| {
            let base = match self.layout {
                PianoLayout::C => (self.octave as i16 + 1) * 12,
                PianoLayout::A => (self.octave as i16 + 1) * 12 - 3,
            };
            (base + off as i16).clamp(0, 127) as u8
        })
    }

    /// Map a keyboard character to a MIDI note offset for C layout.
    fn key_to_offset_c(key: char) -> Option<u8> {
        match key {
            'a' => Some(0),   // C
            's' => Some(2),   // D
            'd' => Some(4),   // E
            'f' => Some(5),   // F
            'g' => Some(7),   // G
            'h' => Some(9),   // A
            'j' => Some(11),  // B
            'w' => Some(1),   // C#
            'e' => Some(3),   // D#
            't' => Some(6),   // F#
            'y' => Some(8),   // G#
            'u' => Some(10),  // A#
            'k' => Some(12),  // C (octave up)
            'l' => Some(14),  // D
            ';' => Some(16),  // E
            'o' => Some(13),  // C#
            'p' => Some(15),  // D#
            _ => None,
        }
    }

    /// Map a keyboard character to a MIDI note offset for A layout.
    fn key_to_offset_a(key: char) -> Option<u8> {
        match key {
            'a' => Some(0),   // A
            's' => Some(2),   // B
            'd' => Some(3),   // C
            'f' => Some(5),   // D
            'g' => Some(7),   // E
            'h' => Some(8),   // F
            'j' => Some(10),  // G
            'w' => Some(1),   // A#
            'e' => Some(4),   // C#
            't' => Some(6),   // D#
            'y' => Some(9),   // F#
            'u' => Some(11),  // G#
            'k' => Some(12),  // A (octave up)
            'l' => Some(14),  // B
            ';' => Some(15),  // C
            'o' => Some(13),  // A#
            'p' => Some(16),  // C#
            _ => None,
        }
    }
}
