use super::automation::AutomationState;
use super::custom_synthdef::CustomSynthDefRegistry;
use super::midi_recording::MidiRecordingState;
use super::music::{Key, Scale};
use super::piano_roll::PianoRollState;
use super::instrument::MixerBus;
use super::vst_plugin::VstPluginRegistry;

pub const MAX_BUSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone)]
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
    pub automation: AutomationState,
    pub midi_recording: MidiRecordingState,
    pub custom_synthdefs: CustomSynthDefRegistry,
    pub vst_plugins: VstPluginRegistry,
    pub buses: Vec<MixerBus>,
    pub master_level: f32,
    pub master_mute: bool,
    pub mixer_selection: MixerSelection,
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
            automation: AutomationState::new(),
            midi_recording: MidiRecordingState::new(),
            custom_synthdefs: CustomSynthDefRegistry::new(),
            vst_plugins: VstPluginRegistry::new(),
            buses,
            master_level: 1.0,
            master_mute: false,
            mixer_selection: MixerSelection::default(),
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
