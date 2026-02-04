#![allow(dead_code)]

use serde::{Serialize, Deserialize};

use super::types::{AutomationLaneId, AutomationPoint, CurveType};
use super::target::AutomationTarget;

/// An automation lane containing points for a single parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationLane {
    pub id: AutomationLaneId,
    pub target: AutomationTarget,
    pub points: Vec<AutomationPoint>,
    pub enabled: bool,
    /// Whether this lane is armed for recording
    pub record_armed: bool,
    /// Minimum value for this parameter
    pub min_value: f32,
    /// Maximum value for this parameter
    pub max_value: f32,
}

impl AutomationLane {
    pub fn new(id: AutomationLaneId, target: AutomationTarget) -> Self {
        let (min_value, max_value) = target.default_range();
        Self {
            id,
            target,
            points: Vec::new(),
            enabled: true,
            record_armed: false,
            min_value,
            max_value,
        }
    }

    /// Add a point at the given tick (inserts in sorted order)
    pub fn add_point(&mut self, tick: u32, value: f32) {
        // Remove existing point at same tick
        self.points.retain(|p| p.tick != tick);

        let point = AutomationPoint::new(tick, value);
        let pos = self.points.iter().position(|p| p.tick > tick).unwrap_or(self.points.len());
        self.points.insert(pos, point);
    }

    /// Remove point at or near the given tick
    pub fn remove_point(&mut self, tick: u32) {
        self.points.retain(|p| p.tick != tick);
    }

    /// Get the interpolated value at a given tick position
    pub fn value_at(&self, tick: u32) -> Option<f32> {
        if self.points.is_empty() || !self.enabled {
            return None;
        }

        // Find surrounding points
        let mut prev: Option<&AutomationPoint> = None;
        let mut next: Option<&AutomationPoint> = None;

        for point in &self.points {
            if point.tick <= tick {
                prev = Some(point);
            } else {
                next = Some(point);
                break;
            }
        }

        let normalized = match (prev, next) {
            (Some(p), None) => p.value,
            (None, Some(n)) => n.value,
            (Some(p), Some(n)) if p.tick == tick => p.value,
            (Some(p), Some(n)) => {
                // Interpolate between p and n
                let t = (tick - p.tick) as f32 / (n.tick - p.tick) as f32;
                self.interpolate(p.value, n.value, t, p.curve)
            }
            (None, None) => return None,
        };

        // Convert from normalized (0-1) to actual value range
        Some(self.min_value + normalized * (self.max_value - self.min_value))
    }

    /// Interpolate between two values based on curve type
    fn interpolate(&self, from: f32, to: f32, t: f32, curve: CurveType) -> f32 {
        match curve {
            CurveType::Linear => from + (to - from) * t,
            CurveType::Step => from,
            CurveType::Exponential => {
                // Exponential interpolation (good for frequency)
                let t_exp = t * t;
                from + (to - from) * t_exp
            }
            CurveType::SCurve => {
                // Smoothstep S-curve
                let t_smooth = t * t * (3.0 - 2.0 * t);
                from + (to - from) * t_smooth
            }
        }
    }

    /// Get the first point at or after the given tick
    pub fn point_at_or_after(&self, tick: u32) -> Option<&AutomationPoint> {
        self.points.iter().find(|p| p.tick >= tick)
    }

    /// Get the last point before the given tick
    pub fn point_before(&self, tick: u32) -> Option<&AutomationPoint> {
        self.points.iter().rev().find(|p| p.tick < tick)
    }

    /// Find point at exact tick
    pub fn point_at(&self, tick: u32) -> Option<&AutomationPoint> {
        self.points.iter().find(|p| p.tick == tick)
    }

    /// Find mutable point at exact tick
    pub fn point_at_mut(&mut self, tick: u32) -> Option<&mut AutomationPoint> {
        self.points.iter_mut().find(|p| p.tick == tick)
    }
}
