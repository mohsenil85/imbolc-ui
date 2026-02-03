#![allow(dead_code)]

use serde::{Serialize, Deserialize};

use crate::state::instrument::InstrumentId;
use super::types::AutomationLaneId;
use super::target::AutomationTarget;
use super::lane::AutomationLane;

/// Collection of automation lanes for a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutomationState {
    pub lanes: Vec<AutomationLane>,
    pub selected_lane: Option<usize>,
    pub(crate) next_lane_id: AutomationLaneId,
}

impl AutomationState {
    pub fn new() -> Self {
        Self {
            lanes: Vec::new(),
            selected_lane: None,
            next_lane_id: 0,
        }
    }

    /// Recalculate next_lane_id from existing lanes (used after loading from DB)
    pub fn recalculate_next_lane_id(&mut self) {
        self.next_lane_id = self.lanes.iter().map(|l| l.id).max().map_or(0, |m| m + 1);
    }

    /// Add a new automation lane for a target
    pub fn add_lane(&mut self, target: AutomationTarget) -> AutomationLaneId {
        // Check if lane already exists for this target
        if let Some(existing) = self.lanes.iter().find(|l| l.target == target) {
            return existing.id;
        }

        let id = self.next_lane_id;
        self.next_lane_id += 1;
        let lane = AutomationLane::new(id, target);
        self.lanes.push(lane);

        if self.selected_lane.is_none() {
            self.selected_lane = Some(self.lanes.len() - 1);
        }

        id
    }

    /// Remove a lane by ID
    pub fn remove_lane(&mut self, id: AutomationLaneId) {
        if let Some(pos) = self.lanes.iter().position(|l| l.id == id) {
            self.lanes.remove(pos);
            // Adjust selection
            if let Some(sel) = self.selected_lane {
                if sel >= self.lanes.len() && !self.lanes.is_empty() {
                    self.selected_lane = Some(self.lanes.len() - 1);
                } else if self.lanes.is_empty() {
                    self.selected_lane = None;
                }
            }
        }
    }

    /// Get lane by ID
    pub fn lane(&self, id: AutomationLaneId) -> Option<&AutomationLane> {
        self.lanes.iter().find(|l| l.id == id)
    }

    /// Get mutable lane by ID
    pub fn lane_mut(&mut self, id: AutomationLaneId) -> Option<&mut AutomationLane> {
        self.lanes.iter_mut().find(|l| l.id == id)
    }

    /// Get lane for a specific target
    pub fn lane_for_target(&self, target: &AutomationTarget) -> Option<&AutomationLane> {
        self.lanes.iter().find(|l| &l.target == target)
    }

    /// Get mutable lane for a specific target
    pub fn lane_for_target_mut(&mut self, target: &AutomationTarget) -> Option<&mut AutomationLane> {
        self.lanes.iter_mut().find(|l| &l.target == target)
    }

    /// Get all lanes for a specific instrument
    pub fn lanes_for_instrument(&self, instrument_id: InstrumentId) -> Vec<&AutomationLane> {
        self.lanes.iter().filter(|l| l.target.instrument_id() == Some(instrument_id)).collect()
    }

    /// Selected lane
    pub fn selected(&self) -> Option<&AutomationLane> {
        self.selected_lane.and_then(|i| self.lanes.get(i))
    }

    /// Selected lane (mutable)
    pub fn selected_mut(&mut self) -> Option<&mut AutomationLane> {
        self.selected_lane.and_then(|i| self.lanes.get_mut(i))
    }

    /// Select next lane
    pub fn select_next(&mut self) {
        if self.lanes.is_empty() {
            self.selected_lane = None;
            return;
        }
        self.selected_lane = match self.selected_lane {
            None => Some(0),
            Some(i) if i + 1 < self.lanes.len() => Some(i + 1),
            Some(i) => Some(i),
        };
    }

    /// Select previous lane
    pub fn select_prev(&mut self) {
        if self.lanes.is_empty() {
            self.selected_lane = None;
            return;
        }
        self.selected_lane = match self.selected_lane {
            None => Some(0),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
        };
    }

    /// Remove all lanes for an instrument (when instrument is deleted)
    pub fn remove_lanes_for_instrument(&mut self, instrument_id: InstrumentId) {
        self.lanes.retain(|l| l.target.instrument_id() != Some(instrument_id));
        // Adjust selection
        if let Some(sel) = self.selected_lane {
            if sel >= self.lanes.len() {
                self.selected_lane = if self.lanes.is_empty() {
                    None
                } else {
                    Some(self.lanes.len() - 1)
                };
            }
        }
    }
}
