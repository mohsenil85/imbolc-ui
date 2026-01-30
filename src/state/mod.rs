pub mod automation;
pub mod custom_synthdef;
pub mod drum_sequencer;
pub mod midi_recording;
pub mod music;
pub mod param;
pub mod persistence;
pub mod piano_roll;
pub mod sampler;
pub mod session;
pub mod strip;
pub mod strip_state;

pub use automation::AutomationTarget;
pub use custom_synthdef::{CustomSynthDef, CustomSynthDefRegistry, ParamSpec};
pub use param::{Param, ParamValue};
pub use sampler::BufferId;
pub use session::{MixerSelection, MusicalSettings, SessionState, MAX_BUSES};
pub use strip::*;
pub use strip_state::StripState;

/// Top-level application state, owned by main.rs and passed to panes by reference.
pub struct AppState {
    pub session: SessionState,
    pub strip: StripState,
    pub audio_in_waveform: Option<Vec<f32>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: SessionState::new(),
            strip: StripState::new(),
            audio_in_waveform: None,
        }
    }

    /// Add a strip, with custom synthdef param setup and piano roll track auto-creation.
    pub fn add_strip(&mut self, source: OscType) -> StripId {
        let id = self.strip.add_strip(source);

        // For custom synthdefs, set params from registry
        if let OscType::Custom(custom_id) = source {
            if let Some(synthdef) = self.session.custom_synthdefs.get(custom_id) {
                if let Some(strip) = self.strip.strip_mut(id) {
                    strip.name = format!("{}-{}", synthdef.synthdef_name, id);
                    strip.source_params = synthdef
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

        // Auto-add piano roll track if strip has_track
        if self.strip.strip(id).map_or(false, |s| s.has_track) {
            self.session.piano_roll.add_track(id);
        }

        id
    }

    /// Remove a strip and its piano roll track.
    pub fn remove_strip(&mut self, id: StripId) {
        self.strip.remove_strip(id);
        self.session.piano_roll.remove_track(id);
    }

    /// Compute effective mute for a strip, considering solo state and master mute.
    pub fn effective_strip_mute(&self, strip: &Strip) -> bool {
        if self.strip.any_strip_solo() {
            !strip.solo
        } else {
            strip.mute || self.session.master_mute
        }
    }

    /// Collect mixer updates for all strips (strip_id, level, mute)
    #[allow(dead_code)]
    pub fn collect_strip_updates(&self) -> Vec<(StripId, f32, bool)> {
        self.strip
            .strips
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.level * self.session.master_level,
                    self.effective_strip_mute(s),
                )
            })
            .collect()
    }

    /// Move mixer selection left/right
    pub fn mixer_move(&mut self, delta: i8) {
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Strip(idx) => {
                let new_idx = (idx as i32 + delta as i32)
                    .clamp(0, self.strip.strips.len().saturating_sub(1) as i32)
                    as usize;
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
        self.session.mixer_selection = match self.session.mixer_selection {
            MixerSelection::Strip(_) => {
                if direction > 0 {
                    MixerSelection::Strip(0)
                } else {
                    MixerSelection::Strip(self.strip.strips.len().saturating_sub(1))
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

    /// Cycle output target for the selected strip
    pub fn mixer_cycle_output(&mut self) {
        if let MixerSelection::Strip(idx) = self.session.mixer_selection {
            if let Some(strip) = self.strip.strips.get_mut(idx) {
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
        if let MixerSelection::Strip(idx) = self.session.mixer_selection {
            if let Some(strip) = self.strip.strips.get_mut(idx) {
                strip.output_target = match strip.output_target {
                    OutputTarget::Master => OutputTarget::Bus(MAX_BUSES as u8),
                    OutputTarget::Bus(1) => OutputTarget::Master,
                    OutputTarget::Bus(n) => OutputTarget::Bus(n - 1),
                };
            }
        }
    }
}
