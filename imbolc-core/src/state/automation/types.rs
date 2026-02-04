#![allow(dead_code)]

use serde::{Serialize, Deserialize};

pub type AutomationLaneId = u32;

/// Interpolation curve type between automation points
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveType {
    /// Linear interpolation (default)
    Linear,
    /// Exponential curve (good for volume, frequency)
    Exponential,
    /// Instant jump (no interpolation)
    Step,
    /// S-curve (smooth transitions)
    SCurve,
}

impl Default for CurveType {
    fn default() -> Self {
        Self::Linear
    }
}

/// A single automation point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationPoint {
    /// Position in ticks
    pub tick: u32,
    /// Normalized value (0.0-1.0, mapped to param's min/max)
    pub value: f32,
    /// Curve type to next point
    pub curve: CurveType,
}

impl AutomationPoint {
    pub fn new(tick: u32, value: f32) -> Self {
        Self {
            tick,
            value: value.clamp(0.0, 1.0),
            curve: CurveType::default(),
        }
    }

    pub fn with_curve(tick: u32, value: f32, curve: CurveType) -> Self {
        Self {
            tick,
            value: value.clamp(0.0, 1.0),
            curve,
        }
    }
}
