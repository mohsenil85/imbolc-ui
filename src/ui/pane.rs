use std::any::Any;

use super::{Graphics, InputEvent, Keymap};
use crate::state::{Connection, ModuleType};

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
    /// Add a module of the given type to the rack
    AddModule(ModuleType),
    /// Delete a module from the rack
    DeleteModule(crate::state::ModuleId),
    /// Request to edit a module (sent by rack pane)
    EditModule(crate::state::ModuleId),
    /// Update a module's params (sent by edit pane when done)
    UpdateModuleParams(crate::state::ModuleId, Vec<crate::state::Param>),
    /// Save rack to file
    SaveRack,
    /// Load rack from file
    LoadRack,
    /// Add a connection between two module ports
    AddConnection(Connection),
    /// Remove a connection between two module ports
    RemoveConnection(Connection),
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
    /// Real-time parameter update
    SetModuleParam(crate::state::ModuleId, String, f32),
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
}

/// Trait for UI panes (screens/views)
pub trait Pane {
    /// Unique identifier for this pane
    fn id(&self) -> &'static str;

    /// Handle an input event, returning an action
    fn handle_input(&mut self, event: InputEvent) -> Action;

    /// Render the pane to the graphics context
    fn render(&self, g: &mut dyn Graphics);

    /// Get the keymap for this pane (for introspection/help)
    fn keymap(&self) -> &Keymap;

    /// Called when this pane becomes active
    fn on_enter(&mut self) {}

    /// Called when this pane becomes inactive
    fn on_exit(&mut self) {}

    /// Handle an action dispatched from elsewhere (e.g., another pane)
    /// Returns true if the action was handled
    fn receive_action(&mut self, _action: &Action) -> bool {
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
    pub fn switch_to(&mut self, id: &str) -> bool {
        if let Some(index) = self.panes.iter().position(|p| p.id() == id) {
            if index != self.active_index {
                self.panes[self.active_index].on_exit();
                self.active_index = index;
                self.panes[self.active_index].on_enter();
            }
            true
        } else {
            false
        }
    }

    /// Handle input for the active pane and process the resulting action
    pub fn handle_input(&mut self, event: InputEvent) -> Action {
        let action = self.active_mut().handle_input(event);

        match &action {
            Action::SwitchPane(id) => {
                self.switch_to(id);
            }
            Action::PushPane(id) => {
                // For push, we'd need a stack - simplified for now
                self.switch_to(id);
            }
            Action::PopPane => {
                // For pop, we'd go back - simplified for now
            }
            _ => {}
        }

        action
    }

    /// Render the active pane
    pub fn render(&self, g: &mut dyn Graphics) {
        self.active().render(g);
    }

    /// Get the keymap of the active pane
    pub fn active_keymap(&self) -> &Keymap {
        self.active().keymap()
    }

    /// Get all registered pane IDs
    pub fn pane_ids(&self) -> Vec<&'static str> {
        self.panes.iter().map(|p| p.id()).collect()
    }

    /// Dispatch an action to a specific pane by ID
    /// Returns true if the pane was found and handled the action
    pub fn dispatch_to(&mut self, id: &str, action: &Action) -> bool {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id() == id) {
            pane.receive_action(action)
        } else {
            false
        }
    }

    /// Get a mutable reference to a pane by ID, downcasted to a specific type
    pub fn get_pane_mut<T: 'static>(&mut self, id: &str) -> Option<&mut T> {
        self.panes
            .iter_mut()
            .find(|p| p.id() == id)
            .and_then(|p| p.as_any_mut().downcast_mut::<T>())
    }
}
