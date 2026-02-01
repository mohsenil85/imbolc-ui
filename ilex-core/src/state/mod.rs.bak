pub mod automation;
pub mod custom_synthdef;
pub mod drum_sequencer;
pub mod instrument;
pub mod instrument_state;
pub mod midi_recording;
pub mod music;
pub mod param;
pub mod persistence;
pub mod piano_roll;
pub mod sampler;
pub mod session;

pub use automation::AutomationTarget;
pub use custom_synthdef::{CustomSynthDef, CustomSynthDefRegistry, ParamSpec};
pub use instrument::*;
pub use instrument_state::InstrumentState;
pub use param::{Param, ParamValue};
pub use sampler::BufferId;
pub use session::{MixerSelection, MusicalSettings, SessionState, MAX_BUSES};

/// Top-level application state, owned by main.rs and passed to panes by reference.
pub struct AppState {
    pub session: SessionState,
    pub instruments: InstrumentState,
    pub audio_in_waveform: Option<Vec<f32>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: SessionState::new(),
            instruments: InstrumentState::new(),
            audio_in_waveform: None,
        }
    }

    /// Add an instrument, with custom synthdef param setup and piano roll track auto-creation.
    pub fn add_instrument(&mut self, source: OscType) -> InstrumentId {
        let id = self.instruments.add_instrument(source);

        // For custom synthdefs, set params from registry
        if let OscType::Custom(custom_id) = source {
            if let Some(synthdef) = self.session.custom_synthdefs.get(custom_id) {
                if let Some(inst) = self.instruments.instrument_mut(id) {
                    inst.name = format!("{}-{}", synthdef.synthdef_name, id);
                    inst.source_params = synthdef
                        .params
                        .iter()
                        .map(|p| param::Param {
                            name: p.name.clone(),
                            value: param::ParamValue::Float(p.default),
                            min: p.min,
                            max: p.max,
                        })
                        .collect();
                }
            }
        }

        // Auto-add piano roll track if instrument has_track
        if self.instruments.instrument(id).map_or(false, |s| s.has_track) {
            self.session.piano_roll.add_track(id);
        }

        id
    }

    /// Remove an instrument and its piano roll track.
    pub fn remove_instrument(&mut self, id: InstrumentId) {
        self.instruments.remove_instrument(id);
        self.session.piano_roll.remove_track(id);
    }

    /// Compute effective mute for an instrument, considering solo state and master mute.
    pub fn effective_instrument_mute(&self, inst: &Instrument) -> bool {
        if self.instruments.any_instrument_solo() {
            !inst.solo
        } else {
            inst.mute || self.session.master_mute
        }
    }

    /// Collect mixer updates for all instruments (instrument_id, level, mute)
    #[allow(dead_code)]
    pub fn collect_instrument_updates(&self) -> Vec<(InstrumentId, f32, bool)> {
        self.instruments
            .instruments
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.level * self.session.master_level,
                    self.effective_instrument_mute(s),
                )
            })
            .collect()
    }

    /// Move mixer selection left/right
    pub fn mixer_move(&mut self, delta: i8) {
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Instrument(idx) => {
                let new_idx = (idx as i32 + delta as i32)
                    .clamp(0, self.instruments.instruments.len().saturating_sub(1) as i32)
                    as usize;
                MixerSelection::Instrument(new_idx)
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
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Instrument(_) => {
                if direction > 0 {
                    MixerSelection::Instrument(0)
                } else {
                    MixerSelection::Instrument(self.instruments.instruments.len().saturating_sub(1))
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

    /// Cycle output target for the selected instrument
    pub fn mixer_cycle_output(&mut self) {
        if let MixerSelection::Instrument(idx) = self.session.mixer_selection {
            if let Some(inst) = self.instruments.instruments.get_mut(idx) {
                inst.output_target = match inst.output_target {
                    OutputTarget::Master => OutputTarget::Bus(1),
                    OutputTarget::Bus(n) if n < MAX_BUSES as u8 => OutputTarget::Bus(n + 1),
                    OutputTarget::Bus(_) => OutputTarget::Master,
                };
            }
        }
    }

    /// Cycle output target backwards for the selected instrument
    pub fn mixer_cycle_output_reverse(&mut self) {
        if let MixerSelection::Instrument(idx) = self.session.mixer_selection {
            if let Some(inst) = self.instruments.instruments.get_mut(idx) {
                inst.output_target = match inst.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }
}
