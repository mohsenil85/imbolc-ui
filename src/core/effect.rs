use std::path::PathBuf;

/// Effects represent side effects to be executed by the runtime
/// Dispatchers return (new_state, Vec<Effect>) to keep them pure
#[derive(Debug, Clone)]
pub enum Effect {
    // Audio (for future SuperCollider integration)
    CreateSynth { module_id: u32 },
    FreeSynth { module_id: u32 },
    SetParam {
        module_id: u32,
        param: String,
        value: f32,
    },

    // Persistence
    Save,
    Load { path: PathBuf },

    // System
    Quit,

    // UI feedback (optional)
    ShowMessage { text: String },
}
