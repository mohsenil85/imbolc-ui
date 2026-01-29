use std::any::Any;
use std::path::PathBuf;

use super::{Graphics, InputEvent, Keymap};
use super::frame::SessionState;
use crate::state::{AppState, EffectType, FilterType, OscType, StripId};

/// Actions that can be returned from pane input handling
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// No action taken, continue as normal
    None,
    /// Quit the application
    Quit,
    /// Switch to a different pane by ID
    SwitchPane(&'static str),
    /// Push a pane onto the stack (for modals/overlays)
    PushPane(&'static str),
    /// Pop the current pane from the stack
    PopPane,
    /// Add a strip with the given oscillator type
    AddStrip(OscType),
    /// Delete a strip
    DeleteStrip(StripId),
    /// Request to edit a strip
    EditStrip(StripId),
    /// Update a strip (save edited strip back)
    UpdateStrip(StripId),
    /// Real-time parameter update on a strip
    SetStripParam(StripId, String, f32),
    /// Add an effect to a strip
    StripAddEffect(StripId, EffectType),
    /// Remove an effect from a strip
    StripRemoveEffect(StripId, usize),
    /// Move an effect up/down in the chain
    StripMoveEffect(StripId, usize, i8),
    /// Set or remove the filter on a strip
    StripSetFilter(StripId, Option<FilterType>),
    /// Toggle piano roll track for a strip
    StripToggleTrack(StripId),
    /// Save state to file
    SaveRack,
    /// Load state from file
    LoadRack,
    /// Connect to audio server
    ConnectServer,
    /// Disconnect from audio server
    DisconnectServer,
    /// Start scsynth server process
    StartServer,
    /// Stop scsynth server process
    StopServer,
    /// Compile synthdefs (slow - runs sclang)
    CompileSynthDefs,
    /// Load pre-compiled synthdefs (fast)
    LoadSynthDefs,
    /// Mixer: move selection left/right
    MixerMove(i8),
    /// Mixer: jump to first (1) or last (-1) in section
    MixerJump(i8),
    /// Mixer: adjust level of selected channel/bus/master
    MixerAdjustLevel(f32),
    /// Mixer: toggle mute on selected
    MixerToggleMute,
    /// Mixer: toggle solo on selected
    MixerToggleSolo,
    /// Mixer: cycle between channels/buses/master sections
    MixerCycleSection,
    /// Mixer: cycle output target for selected channel
    MixerCycleOutput,
    /// Mixer: cycle output target backwards for selected channel
    MixerCycleOutputReverse,
    /// Mixer: adjust send level for a bus
    MixerAdjustSend(u8, f32),
    /// Mixer: toggle send enabled for a bus
    MixerToggleSend(u8),
    /// Piano roll: place or remove a note at cursor
    PianoRollToggleNote,
    /// Piano roll: move cursor (pitch_delta, time_delta)
    PianoRollMoveCursor(i8, i32),
    /// Piano roll: adjust duration of note at cursor
    PianoRollAdjustDuration(i32),
    /// Piano roll: adjust velocity of note at cursor
    PianoRollAdjustVelocity(i8),
    /// Piano roll: toggle play/stop
    PianoRollPlayStop,
    /// Piano roll: toggle loop mode
    PianoRollToggleLoop,
    /// Piano roll: set loop start to cursor position
    PianoRollSetLoopStart,
    /// Piano roll: set loop end to cursor position
    PianoRollSetLoopEnd,
    /// Piano roll: switch to next/prev track
    PianoRollChangeTrack(i8),
    /// Piano roll: set BPM
    PianoRollSetBpm(f32),
    /// Piano roll: zoom time axis
    PianoRollZoom(i8),
    /// Piano roll: scroll vertically by octave
    PianoRollScrollOctave(i8),
    /// Piano roll: jump to start or end
    PianoRollJump(i8),
    /// Piano roll: cycle time signature
    PianoRollCycleTimeSig,
    /// Piano roll: toggle polyphonic/monophonic mode for current track
    PianoRollTogglePolyMode,
    /// Piano roll: play a note immediately from keyboard (pitch, velocity)
    PianoRollPlayNote(u8, u8),
    /// Piano roll: toggle play+record from piano mode
    PianoRollPlayStopRecord,
    /// Strip: play a note on the selected strip (pitch, velocity)
    StripPlayNote(u8, u8),
    /// Strip: select next strip
    StripSelectNext,
    /// Strip: select previous strip
    StripSelectPrev,
    /// Strip: select first strip
    StripSelectFirst,
    /// Strip: select last strip
    StripSelectLast,
    /// Update session state (from frame edit pane)
    UpdateSession(SessionState),
    /// Open file browser for custom synthdef import
    OpenFileBrowser(FileSelectAction),
    /// Import a custom synthdef from a .scd file
    ImportCustomSynthDef(PathBuf),
}

/// Action to take when a file is selected in the file browser
#[derive(Debug, Clone, PartialEq)]
pub enum FileSelectAction {
    ImportCustomSynthDef,
}

/// Trait for UI panes (screens/views).
pub trait Pane {
    /// Unique identifier for this pane
    fn id(&self) -> &'static str;

    /// Handle an input event, returning an action
    fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action;

    /// Render the pane to the graphics context
    fn render(&self, g: &mut dyn Graphics, state: &AppState);

    /// Get the keymap for this pane (for introspection/help)
    fn keymap(&self) -> &Keymap;

    /// Called when this pane becomes active
    fn on_enter(&mut self, _state: &AppState) {}

    /// Called when this pane becomes inactive
    fn on_exit(&mut self, _state: &AppState) {}

    /// Whether this pane is in an input mode that should suppress global keybindings
    fn wants_exclusive_input(&self) -> bool {
        false
    }

    /// Return self as Any for downcasting (required for type-specific access)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Manages a stack of panes with one active pane
pub struct PaneManager {
    panes: Vec<Box<dyn Pane>>,
    active_index: usize,
}

impl PaneManager {
    /// Create a new pane manager with an initial pane
    pub fn new(initial_pane: Box<dyn Pane>) -> Self {
        Self {
            panes: vec![initial_pane],
            active_index: 0,
        }
    }

    /// Add a pane to the manager (does not make it active)
    pub fn add_pane(&mut self, pane: Box<dyn Pane>) {
        self.panes.push(pane);
    }

    /// Get the currently active pane
    pub fn active(&self) -> &dyn Pane {
        self.panes[self.active_index].as_ref()
    }

    /// Get the currently active pane mutably
    pub fn active_mut(&mut self) -> &mut dyn Pane {
        self.panes[self.active_index].as_mut()
    }

    /// Switch to a pane by ID
    pub fn switch_to(&mut self, id: &str, state: &AppState) -> bool {
        if let Some(index) = self.panes.iter().position(|p| p.id() == id) {
            if index != self.active_index {
                self.panes[self.active_index].on_exit(state);
                self.active_index = index;
                self.panes[self.active_index].on_enter(state);
            }
            true
        } else {
            false
        }
    }

    /// Handle input for the active pane and process the resulting action
    pub fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action {
        let action = self.active_mut().handle_input(event, state);

        match &action {
            Action::SwitchPane(id) => {
                self.switch_to(id, state);
            }
            Action::PushPane(id) => {
                self.switch_to(id, state);
            }
            Action::PopPane => {}
            _ => {}
        }

        action
    }

    /// Render the active pane
    pub fn render(&self, g: &mut dyn Graphics, state: &AppState) {
        self.active().render(g, state);
    }

    /// Get the keymap of the active pane
    pub fn active_keymap(&self) -> &Keymap {
        self.active().keymap()
    }

    /// Get all registered pane IDs
    pub fn pane_ids(&self) -> Vec<&'static str> {
        self.panes.iter().map(|p| p.id()).collect()
    }

    /// Get a mutable reference to a pane by ID, downcasted to a specific type
    pub fn get_pane_mut<T: 'static>(&mut self, id: &str) -> Option<&mut T> {
        self.panes
            .iter_mut()
            .find(|p| p.id() == id)
            .and_then(|p| p.as_any_mut().downcast_mut::<T>())
    }
}
