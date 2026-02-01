use super::drum_sequencer::DrumSequencerState;
use super::instrument::*;

#[derive(Debug, Clone)]
pub struct InstrumentState {
    pub instruments: Vec<Instrument>,
    pub selected: Option<usize>,
    pub next_id: InstrumentId,
}

impl InstrumentState {
    pub fn new() -> Self {
        Self {
            instruments: Vec::new(),
            selected: None,
            next_id: 0,
        }
    }

    pub fn add_instrument(&mut self, source: OscType) -> InstrumentId {
        let id = self.next_id;
        self.next_id += 1;
        let instrument = Instrument::new(id, source);
        self.instruments.push(instrument);

        if self.selected.is_none() {
            self.selected = Some(0);
        }

        id
    }

    pub fn remove_instrument(&mut self, id: InstrumentId) {
        if let Some(pos) = self.instruments.iter().position(|s| s.id == id) {
            self.instruments.remove(pos);

            if let Some(sel) = self.selected {
                if sel >= self.instruments.len() {
                    self.selected = if self.instruments.is_empty() {
                        None
                    } else {
                        Some(self.instruments.len() - 1)
                    };
                }
            }
        }
    }

    pub fn instrument(&self, id: InstrumentId) -> Option<&Instrument> {
        self.instruments.iter().find(|s| s.id == id)
    }

    pub fn instrument_mut(&mut self, id: InstrumentId) -> Option<&mut Instrument> {
        self.instruments.iter_mut().find(|s| s.id == id)
    }

    pub fn selected_instrument(&self) -> Option<&Instrument> {
        self.selected.and_then(|idx| self.instruments.get(idx))
    }

    #[allow(dead_code)]
    pub fn selected_instrument_mut(&mut self) -> Option<&mut Instrument> {
        self.selected.and_then(|idx| self.instruments.get_mut(idx))
    }

    pub fn select_next(&mut self) {
        if self.instruments.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(idx) if idx < self.instruments.len() - 1 => Some(idx + 1),
            Some(idx) => Some(idx),
        };
    }

    pub fn select_prev(&mut self) {
        if self.instruments.is_empty() {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(0) => Some(0),
            Some(idx) => Some(idx - 1),
        };
    }

    /// Check if any instrument is soloed
    pub fn any_instrument_solo(&self) -> bool {
        self.instruments.iter().any(|s| s.solo)
    }

    /// Instruments that have tracks (for piano roll)
    #[allow(dead_code)]
    pub fn instruments_with_tracks(&self) -> Vec<&Instrument> {
        self.instruments.iter().filter(|s| s.has_track).collect()
    }

    pub fn selected_drum_sequencer(&self) -> Option<&DrumSequencerState> {
        self.selected_instrument().and_then(|s| s.drum_sequencer.as_ref())
    }

    pub fn selected_drum_sequencer_mut(&mut self) -> Option<&mut DrumSequencerState> {
        self.selected
            .and_then(|idx| self.instruments.get_mut(idx))
            .and_then(|s| s.drum_sequencer.as_mut())
    }
}

impl Default for InstrumentState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_state_creation() {
        let state = InstrumentState::new();
        assert_eq!(state.instruments.len(), 0);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn test_add_instrument() {
        let mut state = InstrumentState::new();
        let id1 = state.add_instrument(OscType::Saw);
        let id2 = state.add_instrument(OscType::Sin);

        assert_eq!(state.instruments.len(), 2);
        assert_eq!(state.instruments[0].id, id1);
        assert_eq!(state.instruments[1].id, id2);
        assert_eq!(state.selected, Some(0));
    }

    #[test]
    fn test_remove_instrument() {
        let mut state = InstrumentState::new();
        let id1 = state.add_instrument(OscType::Saw);
        let id2 = state.add_instrument(OscType::Sin);
        let _id3 = state.add_instrument(OscType::Sqr);

        state.remove_instrument(id2);

        assert_eq!(state.instruments.len(), 2);
        assert_eq!(state.instruments[0].id, id1);
    }

    #[test]
    fn test_remove_last_instrument() {
        let mut state = InstrumentState::new();
        let id1 = state.add_instrument(OscType::Saw);
        let id2 = state.add_instrument(OscType::Sin);

        state.selected = Some(1);
        state.remove_instrument(id2);

        assert_eq!(state.selected, Some(0));
        assert_eq!(state.instruments[0].id, id1);
    }

    #[test]
    fn test_remove_all_instruments() {
        let mut state = InstrumentState::new();
        let id1 = state.add_instrument(OscType::Saw);

        state.remove_instrument(id1);
        assert_eq!(state.selected, None);
        assert!(state.instruments.is_empty());
    }

    #[test]
    fn test_select_navigation() {
        let mut state = InstrumentState::new();
        state.add_instrument(OscType::Saw);
        state.add_instrument(OscType::Sin);
        state.add_instrument(OscType::Sqr);

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
