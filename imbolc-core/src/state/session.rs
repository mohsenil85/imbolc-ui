use super::automation::AutomationState;
use super::arrangement::ArrangementState;
use super::custom_synthdef::CustomSynthDefRegistry;
use super::midi_recording::MidiRecordingState;
use super::music::{Key, Scale};
use super::piano_roll::PianoRollState;
use super::instrument::MixerBus;
use super::vst_plugin::VstPluginRegistry;
use serde::{Serialize, Deserialize};

pub const MAX_BUSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixerSelection {
    Instrument(usize), // index into instruments vec
    Bus(u8),      // 1-8
    Master,
}

impl Default for MixerSelection {
    fn default() -> Self {
        Self::Instrument(0)
    }
}

/// The subset of session fields that are cheap to clone for editing (BPM, key, scale, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicalSettings {
    pub key: Key,
    pub scale: Scale,
    pub bpm: u16,
    pub tuning_a4: f32,
    pub snap: bool,
    pub time_signature: (u8, u8),
}

impl Default for MusicalSettings {
    fn default() -> Self {
        Self {
            key: Key::C,
            scale: Scale::Major,
            bpm: 120,
            tuning_a4: 440.0,
            snap: false,
            time_signature: (4, 4),
        }
    }
}

/// Project-level state container.
/// Owns musical settings, piano roll, automation, mixer buses, and other project data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    // Musical settings (flat, not nested)
    pub key: Key,
    pub scale: Scale,
    pub bpm: u16,
    pub tuning_a4: f32,
    pub snap: bool,
    pub time_signature: (u8, u8),

    // Project state (hoisted from InstrumentState)
    pub piano_roll: PianoRollState,
    pub arrangement: ArrangementState,
    pub automation: AutomationState,
    pub midi_recording: MidiRecordingState,
    pub custom_synthdefs: CustomSynthDefRegistry,
    pub vst_plugins: VstPluginRegistry,
    pub buses: Vec<MixerBus>,
    pub master_level: f32,
    pub master_mute: bool,
    #[serde(skip)]
    pub mixer_selection: MixerSelection,
    /// Global velocity jitter amount (0.0-1.0)
    pub humanize_velocity: f32,
    /// Global timing jitter amount (0.0-1.0)
    pub humanize_timing: f32,
}

impl SessionState {
    pub fn new() -> Self {
        Self::new_with_defaults(MusicalSettings::default())
    }

    pub fn new_with_defaults(defaults: MusicalSettings) -> Self {
        let buses = (1..=MAX_BUSES as u8).map(MixerBus::new).collect();
        Self {
            key: defaults.key,
            scale: defaults.scale,
            bpm: defaults.bpm,
            tuning_a4: defaults.tuning_a4,
            snap: defaults.snap,
            time_signature: defaults.time_signature,
            piano_roll: PianoRollState::new(),
            arrangement: ArrangementState::new(),
            automation: AutomationState::new(),
            midi_recording: MidiRecordingState::new(),
            custom_synthdefs: CustomSynthDefRegistry::new(),
            vst_plugins: VstPluginRegistry::new(),
            buses,
            master_level: 1.0,
            master_mute: false,
            mixer_selection: MixerSelection::default(),
            humanize_velocity: 0.0,
            humanize_timing: 0.0,
        }
    }

    /// Extract the cheap musical settings for editing
    pub fn musical_settings(&self) -> MusicalSettings {
        MusicalSettings {
            key: self.key,
            scale: self.scale,
            bpm: self.bpm,
            tuning_a4: self.tuning_a4,
            snap: self.snap,
            time_signature: self.time_signature,
        }
    }

    /// Apply edited musical settings back
    pub fn apply_musical_settings(&mut self, settings: &MusicalSettings) {
        self.key = settings.key;
        self.scale = settings.scale;
        self.bpm = settings.bpm;
        self.tuning_a4 = settings.tuning_a4;
        self.snap = settings.snap;
        self.time_signature = settings.time_signature;
    }

    pub fn bus(&self, id: u8) -> Option<&MixerBus> {
        self.buses.get((id - 1) as usize)
    }

    pub fn bus_mut(&mut self, id: u8) -> Option<&mut MixerBus> {
        self.buses.get_mut((id - 1) as usize)
    }

    /// Check if any bus is soloed
    pub fn any_bus_solo(&self) -> bool {
        self.buses.iter().any(|b| b.solo)
    }

    /// Compute effective mute for a bus, considering solo state
    pub fn effective_bus_mute(&self, bus: &MixerBus) -> bool {
        if self.any_bus_solo() {
            !bus.solo
        } else {
            bus.mute
        }
    }

    /// Cycle between instrument/bus/master sections
    pub fn mixer_cycle_section(&mut self) {
        self.mixer_selection = match self.mixer_selection {
            MixerSelection::Instrument(_) => MixerSelection::Bus(1),
            MixerSelection::Bus(_) => MixerSelection::Master,
            MixerSelection::Master => MixerSelection::Instrument(0),
        };
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_1based_indexing() {
        let session = SessionState::new();
        assert!(session.bus(1).is_some());
        assert_eq!(session.bus(1).unwrap().id, 1);
        assert!(session.bus(8).is_some());
        assert_eq!(session.bus(8).unwrap().id, 8);
    }

    #[test]
    #[should_panic(expected = "subtract with overflow")]
    fn bus_0_panics() {
        let session = SessionState::new();
        // bus(0) does `id - 1` on u8 0, which panics in debug mode
        let _ = session.bus(0);
    }

    #[test]
    fn bus_out_of_bounds() {
        let session = SessionState::new();
        assert!(session.bus(9).is_none());
    }

    #[test]
    fn effective_bus_mute_no_solo() {
        let session = SessionState::new();
        let bus = session.bus(1).unwrap();
        assert!(!session.effective_bus_mute(bus));

        let mut bus_copy = bus.clone();
        bus_copy.mute = true;
        assert!(session.effective_bus_mute(&bus_copy));
    }

    #[test]
    fn effective_bus_mute_with_solo() {
        let mut session = SessionState::new();
        session.bus_mut(1).unwrap().solo = true;
        // Bus 1 is soloed — should not be muted
        assert!(!session.effective_bus_mute(session.bus(1).unwrap()));
        // Bus 2 is not soloed — should be muted
        assert!(session.effective_bus_mute(session.bus(2).unwrap()));
    }

    #[test]
    fn mixer_cycle_section_full_cycle() {
        let mut session = SessionState::new();
        assert!(matches!(session.mixer_selection, MixerSelection::Instrument(0)));
        session.mixer_cycle_section();
        assert!(matches!(session.mixer_selection, MixerSelection::Bus(1)));
        session.mixer_cycle_section();
        assert!(matches!(session.mixer_selection, MixerSelection::Master));
        session.mixer_cycle_section();
        assert!(matches!(session.mixer_selection, MixerSelection::Instrument(0)));
    }

    #[test]
    fn musical_settings_round_trip() {
        let mut session = SessionState::new();
        session.bpm = 140;
        session.key = Key::D;
        session.scale = Scale::Minor;
        session.tuning_a4 = 442.0;
        session.snap = true;
        session.time_signature = (3, 4);

        let settings = session.musical_settings();
        assert_eq!(settings.bpm, 140);
        assert_eq!(settings.key, Key::D);
        assert_eq!(settings.time_signature, (3, 4));

        // Modify and apply back
        let mut modified = settings.clone();
        modified.bpm = 160;
        modified.key = Key::E;
        session.apply_musical_settings(&modified);
        assert_eq!(session.bpm, 160);
        assert_eq!(session.key, Key::E);
    }

    #[test]
    fn any_bus_solo() {
        let mut session = SessionState::new();
        assert!(!session.any_bus_solo());
        session.bus_mut(3).unwrap().solo = true;
        assert!(session.any_bus_solo());
    }
}
