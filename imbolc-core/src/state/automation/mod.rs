mod types;
mod target;
mod lane;
mod state;
mod tests;

pub use types::{AutomationLaneId, CurveType, AutomationPoint};
pub use target::AutomationTarget;
pub use lane::AutomationLane;
pub use state::AutomationState;
