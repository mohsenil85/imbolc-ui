mod editing;
mod input;
mod rendering;

use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;

use crate::state::{
    AppState, EffectSlot, EnvConfig, FilterConfig, Instrument, InstrumentId, LfoConfig,
    Param, SourceType,
};
use crate::ui::widgets::TextInput;
use crate::ui::{Action, InputEvent, Keymap, MouseEvent, Pane, PianoKeyboard, ToggleResult};

/// Which section a row belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Source,
    Filter,
    Effects,
    Lfo,
    Envelope,
}

pub struct InstrumentEditPane {
    keymap: Keymap,
    instrument_id: Option<InstrumentId>,
    instrument_name: String,
    source: SourceType,
    source_params: Vec<Param>,
    sample_name: Option<String>,
    filter: Option<FilterConfig>,
    effects: Vec<EffectSlot>,
    lfo: LfoConfig,
    amp_envelope: EnvConfig,
    polyphonic: bool,
    active: bool,
    pub(crate) selected_row: usize,
    editing: bool,
    edit_input: TextInput,
    piano: PianoKeyboard,
}

impl InstrumentEditPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            instrument_id: None,
            instrument_name: String::new(),
            source: SourceType::Saw,
            source_params: Vec::new(),
            sample_name: None,
            filter: None,
            effects: Vec::new(),
            lfo: LfoConfig::default(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            active: true,
            selected_row: 0,
            editing: false,
            edit_input: TextInput::new(""),
            piano: PianoKeyboard::new(),
        }
    }

    #[allow(dead_code)]
    pub fn set_instrument(&mut self, instrument: &Instrument) {
        self.instrument_id = Some(instrument.id);
        self.instrument_name = instrument.name.clone();
        self.source = instrument.source;
        self.source_params = instrument.source_params.clone();
        self.sample_name = instrument.sampler_config.as_ref().and_then(|c| c.sample_name.clone());
        self.filter = instrument.filter.clone();
        self.effects = instrument.effects.clone();
        self.lfo = instrument.lfo.clone();
        self.amp_envelope = instrument.amp_envelope.clone();
        self.polyphonic = instrument.polyphonic;
        self.active = instrument.active;
        self.selected_row = 0;
    }

    #[allow(dead_code)]
    pub fn instrument_id(&self) -> Option<InstrumentId> {
        self.instrument_id
    }

    /// Get current tab as index (for view state - now section based)
    pub fn tab_index(&self) -> u8 {
        match self.current_section() {
            Section::Source => 0,
            Section::Filter => 1,
            Section::Effects => 2,
            Section::Lfo => 3,
            Section::Envelope => 4,
        }
    }

    /// Set tab from index (for view state restoration)
    pub fn set_tab_index(&mut self, idx: u8) {
        let target_section = match idx {
            0 => Section::Source,
            1 => Section::Filter,
            2 => Section::Effects,
            3 => Section::Lfo,
            4 => Section::Envelope,
            _ => Section::Source,
        };
        for i in 0..self.total_rows() {
            if self.section_for_row(i) == target_section {
                self.selected_row = i;
                break;
            }
        }
    }

    /// Apply edits back to an instrument
    #[allow(dead_code)]
    pub fn apply_to(&self, instrument: &mut Instrument) {
        instrument.source = self.source;
        instrument.source_params = self.source_params.clone();
        instrument.filter = self.filter.clone();
        instrument.effects = self.effects.clone();
        instrument.lfo = self.lfo.clone();
        instrument.amp_envelope = self.amp_envelope.clone();
        instrument.polyphonic = self.polyphonic;
        instrument.active = self.active;
    }

    /// Total number of selectable rows across all sections
    fn total_rows(&self) -> usize {
        let sample_row = if self.source.is_sample() { 1 } else { 0 };
        let source_rows = sample_row + self.source_params.len().max(1);
        let filter_rows = if self.filter.is_some() { 3 } else { 1 };
        let effect_rows = self.effects.len().max(1);
        let lfo_rows = 4;
        let env_rows = if self.source.is_vst() { 0 } else { 4 };
        source_rows + filter_rows + effect_rows + lfo_rows + env_rows
    }

    /// Which section does a given row belong to?
    fn section_for_row(&self, row: usize) -> Section {
        let sample_row = if self.source.is_sample() { 1 } else { 0 };
        let source_rows = sample_row + self.source_params.len().max(1);
        let filter_rows = if self.filter.is_some() { 3 } else { 1 };
        let effect_rows = self.effects.len().max(1);
        let lfo_rows = 4;

        if row < source_rows {
            Section::Source
        } else if row < source_rows + filter_rows {
            Section::Filter
        } else if row < source_rows + filter_rows + effect_rows {
            Section::Effects
        } else if row < source_rows + filter_rows + effect_rows + lfo_rows {
            Section::Lfo
        } else {
            Section::Envelope
        }
    }

    /// Get section and local index for a row
    fn row_info(&self, row: usize) -> (Section, usize) {
        let sample_row = if self.source.is_sample() { 1 } else { 0 };
        let source_rows = sample_row + self.source_params.len().max(1);
        let filter_rows = if self.filter.is_some() { 3 } else { 1 };
        let effect_rows = self.effects.len().max(1);
        let lfo_rows = 4;

        if row < source_rows {
            (Section::Source, row)
        } else if row < source_rows + filter_rows {
            (Section::Filter, row - source_rows)
        } else if row < source_rows + filter_rows + effect_rows {
            (Section::Effects, row - source_rows - filter_rows)
        } else if row < source_rows + filter_rows + effect_rows + lfo_rows {
            (Section::Lfo, row - source_rows - filter_rows - effect_rows)
        } else {
            (Section::Envelope, row - source_rows - filter_rows - effect_rows - lfo_rows)
        }
    }

    fn current_section(&self) -> Section {
        self.section_for_row(self.selected_row)
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }
}

impl Pane for InstrumentEditPane {
    fn id(&self) -> &'static str {
        "instrument_edit"
    }

    fn handle_action(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
        self.handle_action_impl(action, event, state)
    }

    fn handle_raw_input(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        self.handle_raw_input_impl(event);
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_impl(area, buf, state);
    }

    fn handle_mouse(&mut self, event: &MouseEvent, _area: RatatuiRect, _state: &AppState) -> Action {
        self.handle_mouse_impl(event)
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn toggle_performance_mode(&mut self, _state: &AppState) -> ToggleResult {
        if self.piano.is_active() {
            self.piano.handle_escape();
            if self.piano.is_active() {
                ToggleResult::CycledLayout
            } else {
                ToggleResult::Deactivated
            }
        } else {
            self.piano.activate();
            ToggleResult::ActivatedPiano
        }
    }

    fn activate_piano(&mut self) {
        if !self.piano.is_active() { self.piano.activate(); }
    }

    fn deactivate_performance(&mut self) {
        self.piano.deactivate();
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Default for InstrumentEditPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}
