use std::any::Any;
use std::path::PathBuf;

use super::{Graphics, InputEvent, Keymap};
use crate::state::{AppState, EffectType, FilterType, MusicalSettings, OscType, StripId};

/// Drum sequencer actions
#[derive(Debug, Clone, PartialEq)]
pub enum SequencerAction {
    ToggleStep(usize, usize),         // (pad_idx, step_idx)
    AdjustVelocity(usize, usize, i8), // (pad_idx, step_idx, delta)
    PlayStop,
    LoadSample(usize),              // pad_idx
    ClearPad(usize),                // pad_idx
    ClearPattern,
    CyclePatternLength,
    NextPattern,
    PrevPattern,
    AdjustPadLevel(usize, f32),     // (pad_idx, delta)
    LoadSampleResult(usize, PathBuf), // (pad_idx, path) — from file browser
}

/// Navigation actions (pane switching, modal stack)
#[derive(Debug, Clone, PartialEq)]
pub enum NavAction {
    SwitchPane(&'static str),
    #[allow(dead_code)]
    PushPane(&'static str),
    PopPane,
}

/// Strip/instrument actions
#[derive(Debug, Clone, PartialEq)]
pub enum StripAction {
    Add(OscType),
    Delete(StripId),
    Edit(StripId),
    Update(StripId),
    #[allow(dead_code)]
    SetParam(StripId, String, f32),
    #[allow(dead_code)]
    AddEffect(StripId, EffectType),
    #[allow(dead_code)]
    RemoveEffect(StripId, usize),
    #[allow(dead_code)]
    MoveEffect(StripId, usize, i8),
    #[allow(dead_code)]
    SetFilter(StripId, Option<FilterType>),
    #[allow(dead_code)]
    ToggleTrack(StripId),
    PlayNote(u8, u8),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
}

/// Mixer actions
#[derive(Debug, Clone, PartialEq)]
pub enum MixerAction {
    Move(i8),
    Jump(i8),
    AdjustLevel(f32),
    ToggleMute,
    ToggleSolo,
    CycleSection,
    CycleOutput,
    CycleOutputReverse,
    AdjustSend(u8, f32),
    ToggleSend(u8),
}

/// Piano roll actions
#[derive(Debug, Clone, PartialEq)]
pub enum PianoRollAction {
    ToggleNote,
    #[allow(dead_code)]
    MoveCursor(i8, i32),
    AdjustDuration(i32),
    AdjustVelocity(i8),
    PlayStop,
    ToggleLoop,
    SetLoopStart,
    SetLoopEnd,
    ChangeTrack(i8),
    #[allow(dead_code)]
    SetBpm(f32),
    #[allow(dead_code)]
    Zoom(i8),
    #[allow(dead_code)]
    ScrollOctave(i8),
    Jump(i8),
    CycleTimeSig,
    TogglePolyMode,
    PlayNote(u8, u8),
    PlayStopRecord,
}

/// Audio server actions
#[derive(Debug, Clone, PartialEq)]
pub enum ServerAction {
    Connect,
    Disconnect,
    Start,
    Stop,
    CompileSynthDefs,
    LoadSynthDefs,
}

/// Session/file actions
#[derive(Debug, Clone, PartialEq)]
pub enum SessionAction {
    Save,
    Load,
    UpdateSession(MusicalSettings),
    OpenFileBrowser(FileSelectAction),
    ImportCustomSynthDef(PathBuf),
}

/// Actions that can be returned from pane input handling
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Quit,
    Nav(NavAction),
    Strip(StripAction),
    Mixer(MixerAction),
    PianoRoll(PianoRollAction),
    Server(ServerAction),
    Session(SessionAction),
    Sequencer(SequencerAction),
}

/// Action to take when a file is selected in the file browser
#[derive(Debug, Clone, PartialEq)]
pub enum FileSelectAction {
    ImportCustomSynthDef,
    LoadDrumSample(usize), // pad index
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
    stack: Vec<usize>,
}

impl PaneManager {
    /// Create a new pane manager with an initial pane
    pub fn new(initial_pane: Box<dyn Pane>) -> Self {
        Self {
            panes: vec![initial_pane],
            active_index: 0,
            stack: Vec::new(),
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

    /// Switch to a pane by ID (flat navigation — clears the stack)
    pub fn switch_to(&mut self, id: &str, state: &AppState) -> bool {
        if let Some(index) = self.panes.iter().position(|p| p.id() == id) {
            if index != self.active_index {
                self.panes[self.active_index].on_exit(state);
                self.active_index = index;
                self.panes[self.active_index].on_enter(state);
            }
            self.stack.clear();
            true
        } else {
            false
        }
    }

    /// Push current pane onto the stack and switch to a new pane (for modals/overlays)
    pub fn push_to(&mut self, id: &str, state: &AppState) -> bool {
        if let Some(index) = self.panes.iter().position(|p| p.id() == id) {
            self.stack.push(self.active_index);
            self.panes[self.active_index].on_exit(state);
            self.active_index = index;
            self.panes[self.active_index].on_enter(state);
            true
        } else {
            false
        }
    }

    /// Pop the stack and return to the previous pane
    pub fn pop(&mut self, state: &AppState) -> bool {
        if let Some(prev_index) = self.stack.pop() {
            self.panes[self.active_index].on_exit(state);
            self.active_index = prev_index;
            self.panes[self.active_index].on_enter(state);
            true
        } else {
            false
        }
    }

    /// Handle input for the active pane and process the resulting action
    pub fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action {
        let action = self.active_mut().handle_input(event, state);

        match &action {
            Action::Nav(NavAction::SwitchPane(id)) => {
                self.switch_to(id, state);
            }
            Action::Nav(NavAction::PushPane(id)) => {
                self.push_to(id, state);
            }
            Action::Nav(NavAction::PopPane) => {
                self.pop(state);
            }
            _ => {}
        }

        action
    }

    /// Render the active pane
    pub fn render(&self, g: &mut dyn Graphics, state: &AppState) {
        self.active().render(g, state);
    }

    /// Get the keymap of the active pane
    #[allow(dead_code)]
    pub fn active_keymap(&self) -> &Keymap {
        self.active().keymap()
    }

    /// Get all registered pane IDs
    #[allow(dead_code)]
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
