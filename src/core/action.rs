// TODO: Uncomment when state::ModuleType is available (Task 1)
// use crate::state::ModuleType;

/// Actions represent user intentions that modify state
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Navigation
    MoveUp,
    MoveDown,
    SelectNext,  // n key
    SelectPrev,  // p key
    GotoFirst,   // g key
    GotoLast,    // G key

    // Module operations
    // TODO: Uncomment when ModuleType is available
    // AddModule(ModuleType),
    DeleteSelected,
    EditSelected,

    // Parameter editing (in edit view)
    ParamIncrement,
    ParamDecrement,
    ParamSet(f32),
    NextParam,
    PrevParam,

    // View switching
    OpenAddView,
    OpenEditView,
    CloseView, // Escape - close modal/go back
    Confirm,   // Enter - confirm selection

    // System
    Quit,
    Save,
    Undo,
    Redo,
}
