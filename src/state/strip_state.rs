use serde::{Deserialize, Serialize};

use super::strip::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripState {
    pub strips: Vec<Strip>,
    pub selected: Option<usize>,
    pub next_id: StripId,
}

impl StripState {
    pub fn new() -> Self {
        Self {
            strips: Vec::new(),
            selected: None,
            next_id: 0,
        }
    }

    pub fn add_strip(&mut self, source: OscType) -> StripId {
        let id = self.next_id;
        self.next_id += 1;
        let strip = Strip::new(id, source);
        self.strips.push(strip);

        if self.selected.is_none() {
            self.selected = Some(0);
        }

        id
    }

    pub fn remove_strip(&mut self, id: StripId) {
        if let Some(pos) = self.strips.iter().position(|s| s.id == id) {
            self.strips.remove(pos);

            if let Some(sel) = self.selected {
                if sel >= self.strips.len() {
                    self.selected = if self.strips.is_empty() {
                        None
                    } else {
                        Some(self.strips.len() - 1)
                    };
                }
            }
        }
    }

    pub fn strip(&self, id: StripId) -> Option<&Strip> {
        self.strips.iter().find(|s| s.id == id)
    }

    pub fn strip_mut(&mut self, id: StripId) -> Option<&mut Strip> {
        self.strips.iter_mut().find(|s| s.id == id)
    }

    pub fn selected_strip(&self) -> Option<&Strip> {
        self.selected.and_then(|idx| self.strips.get(idx))
    }

    #[allow(dead_code)]
    pub fn selected_strip_mut(&mut self) -> Option<&mut Strip> {
        self.selected.and_then(|idx| self.strips.get_mut(idx))
    }

    pub fn select_next(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(idx) if idx < self.strips.len() - 1 => Some(idx + 1),
            Some(idx) => Some(idx),
        };
    }

    pub fn select_prev(&mut self) {
        if self.strips.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(0) => Some(0),
            Some(idx) => Some(idx - 1),
        };
    }

    /// Check if any strip is soloed
    pub fn any_strip_solo(&self) -> bool {
        self.strips.iter().any(|s| s.solo)
    }

    /// Strips that have tracks (for piano roll)
    #[allow(dead_code)]
    pub fn strips_with_tracks(&self) -> Vec<&Strip> {
        self.strips.iter().filter(|s| s.has_track).collect()
    }
}

impl Default for StripState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_state_creation() {
        let state = StripState::new();
        assert_eq!(state.strips.len(), 0);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn test_add_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
        assert_eq!(state.strips[1].id, id2);
        assert_eq!(state.selected, Some(0));
    }

    #[test]
    fn test_remove_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);
        let _id3 = state.add_strip(OscType::Sqr);

        state.remove_strip(id2);

        assert_eq!(state.strips.len(), 2);
        assert_eq!(state.strips[0].id, id1);
    }

    #[test]
    fn test_remove_last_strip() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);
        let id2 = state.add_strip(OscType::Sin);

        state.selected = Some(1);
        state.remove_strip(id2);

        assert_eq!(state.selected, Some(0));
        assert_eq!(state.strips[0].id, id1);
    }

    #[test]
    fn test_remove_all_strips() {
        let mut state = StripState::new();
        let id1 = state.add_strip(OscType::Saw);

        state.remove_strip(id1);
        assert_eq!(state.selected, None);
        assert!(state.strips.is_empty());
    }

    #[test]
    fn test_select_navigation() {
        let mut state = StripState::new();
        state.add_strip(OscType::Saw);
        state.add_strip(OscType::Sin);
        state.add_strip(OscType::Sqr);

        assert_eq!(state.selected, Some(0));
        state.select_next();
        assert_eq!(state.selected, Some(1));
        state.select_next();
        assert_eq!(state.selected, Some(2));
        state.select_next();
        assert_eq!(state.selected, Some(2)); // stay at end
        state.select_prev();
        assert_eq!(state.selected, Some(1));
        state.select_prev();
        assert_eq!(state.selected, Some(0));
        state.select_prev();
        assert_eq!(state.selected, Some(0)); // stay at start
    }
}
