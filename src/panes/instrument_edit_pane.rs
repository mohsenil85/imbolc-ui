use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{
    AppState, EffectSlot, EffectType, EnvConfig, FilterConfig, FilterType, LfoConfig,
    SourceType, Param, ParamValue, InstrumentId, Instrument,
};
use crate::ui::layout_helpers::center_rect;
use crate::ui::widgets::TextInput;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, Pane, PianoKeyboard, InstrumentAction, Style};

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
    filter: Option<FilterConfig>,
    effects: Vec<EffectSlot>,
    lfo: LfoConfig,
    amp_envelope: EnvConfig,
    polyphonic: bool,
    selected_row: usize,
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
            filter: None,
            effects: Vec::new(),
            lfo: LfoConfig::default(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            selected_row: 0,
            editing: false,
            edit_input: TextInput::new(""),
            piano: PianoKeyboard::new(),
        }
    }

    pub fn set_instrument(&mut self, instrument: &Instrument) {
        self.instrument_id = Some(instrument.id);
        self.instrument_name = instrument.name.clone();
        self.source = instrument.source;
        self.source_params = instrument.source_params.clone();
        self.filter = instrument.filter.clone();
        self.effects = instrument.effects.clone();
        self.lfo = instrument.lfo.clone();
        self.amp_envelope = instrument.amp_envelope.clone();
        self.polyphonic = instrument.polyphonic;
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
        // Jump to first row of the section
        let target_section = match idx {
            0 => Section::Source,
            1 => Section::Filter,
            2 => Section::Effects,
            3 => Section::Lfo,
            4 => Section::Envelope,
            _ => Section::Source,
        };
        // Find first row of that section
        for i in 0..self.total_rows() {
            if self.section_for_row(i) == target_section {
                self.selected_row = i;
                break;
            }
        }
    }

    /// Apply edits back to an instrument
    pub fn apply_to(&self, instrument: &mut Instrument) {
        instrument.source = self.source;
        instrument.source_params = self.source_params.clone();
        instrument.filter = self.filter.clone();
        instrument.effects = self.effects.clone();
        instrument.lfo = self.lfo.clone();
        instrument.amp_envelope = self.amp_envelope.clone();
        instrument.polyphonic = self.polyphonic;
    }

    /// Total number of selectable rows across all sections
    fn total_rows(&self) -> usize {
        let source_rows = self.source_params.len().max(1); // At least 1 for empty message
        let filter_rows = if self.filter.is_some() { 3 } else { 1 }; // type/cutoff/res or "off"
        let effect_rows = self.effects.len().max(1); // At least 1 for empty message
        let lfo_rows = 4; // enabled, rate, depth, shape/target
        let env_rows = 4; // A, D, S, R
        source_rows + filter_rows + effect_rows + lfo_rows + env_rows
    }

    /// Which section does a given row belong to?
    fn section_for_row(&self, row: usize) -> Section {
        let source_rows = self.source_params.len().max(1);
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
        let source_rows = self.source_params.len().max(1);
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

    fn adjust_value(&mut self, increase: bool, big: bool) {
        let (section, local_idx) = self.row_info(self.selected_row);
        let fraction = if big { 0.10 } else { 0.05 };

        match section {
            Section::Source => {
                if let Some(param) = self.source_params.get_mut(local_idx) {
                    adjust_param(param, increase, fraction);
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    match local_idx {
                        0 => {} // type - use 't' to cycle
                        1 => {
                            let range = f.cutoff.max - f.cutoff.min;
                            let delta = range * fraction;
                            if increase { f.cutoff.value = (f.cutoff.value + delta).min(f.cutoff.max); }
                            else { f.cutoff.value = (f.cutoff.value - delta).max(f.cutoff.min); }
                        }
                        2 => {
                            let range = f.resonance.max - f.resonance.min;
                            let delta = range * fraction;
                            if increase { f.resonance.value = (f.resonance.value + delta).min(f.resonance.max); }
                            else { f.resonance.value = (f.resonance.value - delta).max(f.resonance.min); }
                        }
                        _ => {}
                    }
                }
            }
            Section::Effects => {
                if let Some(effect) = self.effects.get_mut(local_idx) {
                    if let Some(param) = effect.params.first_mut() {
                        adjust_param(param, increase, fraction);
                    }
                }
            }
            Section::Lfo => {
                match local_idx {
                    0 => {} // enabled - use 'l' to toggle
                    1 => {
                        // rate: 0.1 to 32 Hz
                        let delta = if big { 2.0 } else { 0.5 };
                        if increase { self.lfo.rate = (self.lfo.rate + delta).min(32.0); }
                        else { self.lfo.rate = (self.lfo.rate - delta).max(0.1); }
                    }
                    2 => {
                        // depth: 0 to 1
                        let delta = fraction;
                        if increase { self.lfo.depth = (self.lfo.depth + delta).min(1.0); }
                        else { self.lfo.depth = (self.lfo.depth - delta).max(0.0); }
                    }
                    3 => {} // shape/target - use 's'/'m' to cycle
                    _ => {}
                }
            }
            Section::Envelope => {
                let delta = if big { 0.1 } else { 0.05 };
                let val = match local_idx {
                    0 => &mut self.amp_envelope.attack,
                    1 => &mut self.amp_envelope.decay,
                    2 => &mut self.amp_envelope.sustain,
                    3 => &mut self.amp_envelope.release,
                    _ => return,
                };
                if increase { *val = (*val + delta).min(if local_idx == 2 { 1.0 } else { 5.0 }); }
                else { *val = (*val - delta).max(0.0); }
            }
        }
    }

    fn emit_update(&self) -> Action {
        if let Some(id) = self.instrument_id {
            Action::Instrument(InstrumentAction::Update(id))
        } else {
            Action::None
        }
    }

    /// Set current parameter to its minimum (zero) value
    fn zero_current_param(&mut self) {
        let (section, local_idx) = self.row_info(self.selected_row);

        match section {
            Section::Source => {
                if let Some(param) = self.source_params.get_mut(local_idx) {
                    zero_param(param);
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    match local_idx {
                        0 => {} // type - can't zero
                        1 => f.cutoff.value = f.cutoff.min,
                        2 => f.resonance.value = f.resonance.min,
                        _ => {}
                    }
                }
            }
            Section::Effects => {
                if let Some(effect) = self.effects.get_mut(local_idx) {
                    if let Some(param) = effect.params.first_mut() {
                        zero_param(param);
                    }
                }
            }
            Section::Lfo => {
                match local_idx {
                    0 => self.lfo.enabled = false,
                    1 => self.lfo.rate = 0.1,
                    2 => self.lfo.depth = 0.0,
                    3 => {} // shape/target - can't zero
                    _ => {}
                }
            }
            Section::Envelope => {
                match local_idx {
                    0 => self.amp_envelope.attack = 0.0,
                    1 => self.amp_envelope.decay = 0.0,
                    2 => self.amp_envelope.sustain = 0.0,
                    3 => self.amp_envelope.release = 0.0,
                    _ => {}
                }
            }
        }
    }

    /// Set all parameters in the current section to their minimum values
    fn zero_current_section(&mut self) {
        let section = self.current_section();

        match section {
            Section::Source => {
                for param in &mut self.source_params {
                    zero_param(param);
                }
            }
            Section::Filter => {
                if let Some(ref mut f) = self.filter {
                    f.cutoff.value = f.cutoff.min;
                    f.resonance.value = f.resonance.min;
                }
            }
            Section::Effects => {
                for effect in &mut self.effects {
                    for param in &mut effect.params {
                        zero_param(param);
                    }
                }
            }
            Section::Lfo => {
                self.lfo.enabled = false;
                self.lfo.rate = 0.1;
                self.lfo.depth = 0.0;
            }
            Section::Envelope => {
                self.amp_envelope.attack = 0.0;
                self.amp_envelope.decay = 0.0;
                self.amp_envelope.sustain = 0.0;
                self.amp_envelope.release = 0.0;
            }
        }
    }

}

fn adjust_param(param: &mut Param, increase: bool, fraction: f32) {
    let range = param.max - param.min;
    match &mut param.value {
        ParamValue::Float(ref mut v) => {
            let delta = range * fraction;
            if increase { *v = (*v + delta).min(param.max); }
            else { *v = (*v - delta).max(param.min); }
        }
        ParamValue::Int(ref mut v) => {
            let delta = ((range * fraction) as i32).max(1);
            if increase { *v = (*v + delta).min(param.max as i32); }
            else { *v = (*v - delta).max(param.min as i32); }
        }
        ParamValue::Bool(ref mut v) => { *v = !*v; }
    }
}

fn zero_param(param: &mut Param) {
    match &mut param.value {
        ParamValue::Float(ref mut v) => *v = param.min,
        ParamValue::Int(ref mut v) => *v = param.min as i32,
        ParamValue::Bool(ref mut v) => *v = false,
    }
}

fn render_slider(value: f32, min: f32, max: f32, width: usize) -> String {
    let normalized = (value - min) / (max - min);
    let pos = (normalized * width as f32) as usize;
    let pos = pos.min(width);
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        if i == pos { s.push('|'); }
        else if i < pos { s.push('='); }
        else { s.push('-'); }
    }
    s.push(']');
    s
}

impl Pane for InstrumentEditPane {
    fn id(&self) -> &'static str {
        "instrument_edit"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        // Piano mode
        if self.piano.is_active() {
            match event.key {
                KeyCode::Char('[') => {
                    self.piano.octave_down();
                    return Action::None;
                }
                KeyCode::Char(']') => {
                    self.piano.octave_up();
                    return Action::None;
                }
                KeyCode::Up => {
                    if self.selected_row > 0 { self.selected_row -= 1; }
                    return Action::None;
                }
                KeyCode::Down => {
                    let total = self.total_rows();
                    if self.selected_row + 1 < total { self.selected_row += 1; }
                    return Action::None;
                }
                KeyCode::Left => {
                    self.adjust_value(false, false);
                    return self.emit_update();
                }
                KeyCode::Right => {
                    self.adjust_value(true, false);
                    return self.emit_update();
                }
                KeyCode::Char('\\') => {
                    self.zero_current_param();
                    return self.emit_update();
                }
                KeyCode::Char('|') => {
                    self.zero_current_section();
                    return self.emit_update();
                }
                KeyCode::Char(c) => {
                    if let Some(pitch) = self.piano.key_to_pitch(c) {
                        let velocity = if event.modifiers.shift { 127 } else { 100 };
                        return Action::Instrument(InstrumentAction::PlayNote(pitch, velocity));
                    }
                    return Action::None;
                }
                _ => return Action::None,
            }
        }

        // Text editing mode
        if self.editing {
            match event.key {
                KeyCode::Enter => {
                    let text = self.edit_input.value().to_string();
                    let (section, local_idx) = self.row_info(self.selected_row);
                    match section {
                        Section::Source => {
                            if let Some(param) = self.source_params.get_mut(local_idx) {
                                if let Ok(v) = text.parse::<f32>() {
                                    param.value = ParamValue::Float(v.clamp(param.min, param.max));
                                }
                            }
                        }
                        Section::Filter => {
                            if let Some(ref mut f) = self.filter {
                                match local_idx {
                                    1 => if let Ok(v) = text.parse::<f32>() { f.cutoff.value = v.clamp(f.cutoff.min, f.cutoff.max); },
                                    2 => if let Ok(v) = text.parse::<f32>() { f.resonance.value = v.clamp(f.resonance.min, f.resonance.max); },
                                    _ => {}
                                }
                            }
                        }
                        Section::Envelope => {
                            if let Ok(v) = text.parse::<f32>() {
                                let max = if local_idx == 2 { 1.0 } else { 5.0 };
                                let val = v.clamp(0.0, max);
                                match local_idx {
                                    0 => self.amp_envelope.attack = val,
                                    1 => self.amp_envelope.decay = val,
                                    2 => self.amp_envelope.sustain = val,
                                    3 => self.amp_envelope.release = val,
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    self.editing = false;
                    self.edit_input.set_focused(false);
                    return self.emit_update();
                }
                KeyCode::Escape => {
                    self.editing = false;
                    self.edit_input.set_focused(false);
                    return Action::None;
                }
                _ => {
                    self.edit_input.handle_input(&event);
                    return Action::None;
                }
            }
        }

        match self.keymap.lookup(&event) {
            Some("done") => {
                return self.emit_update();
            }
            Some("next") => {
                let total = self.total_rows();
                if total > 0 {
                    self.selected_row = (self.selected_row + 1) % total;
                }
            }
            Some("prev") => {
                let total = self.total_rows();
                if total > 0 {
                    self.selected_row = if self.selected_row == 0 { total - 1 } else { self.selected_row - 1 };
                }
            }
            Some("increase") => {
                self.adjust_value(true, false);
                return self.emit_update();
            }
            Some("decrease") => {
                self.adjust_value(false, false);
                return self.emit_update();
            }
            Some("increase_big") => {
                self.adjust_value(true, true);
                return self.emit_update();
            }
            Some("decrease_big") => {
                self.adjust_value(false, true);
                return self.emit_update();
            }
            Some("enter_edit") => {
                self.editing = true;
                self.edit_input.set_value("");
                self.edit_input.set_focused(true);
            }
            Some("toggle_filter") => {
                if self.filter.is_some() {
                    self.filter = None;
                } else {
                    self.filter = Some(FilterConfig::new(FilterType::Lpf));
                }
                return self.emit_update();
            }
            Some("cycle_filter_type") => {
                if let Some(ref mut f) = self.filter {
                    f.filter_type = match f.filter_type {
                        FilterType::Lpf => FilterType::Hpf,
                        FilterType::Hpf => FilterType::Bpf,
                        FilterType::Bpf => FilterType::Lpf,
                    };
                    return self.emit_update();
                }
            }
            Some("add_effect") => {
                let next_type = if self.effects.is_empty() {
                    EffectType::Delay
                } else {
                    match self.effects.last().unwrap().effect_type {
                        EffectType::Delay => EffectType::Reverb,
                        EffectType::Reverb => EffectType::Gate,
                        EffectType::Gate => EffectType::Delay,
                    }
                };
                self.effects.push(EffectSlot::new(next_type));
                return self.emit_update();
            }
            Some("remove_effect") => {
                let (section, local_idx) = self.row_info(self.selected_row);
                if section == Section::Effects && !self.effects.is_empty() {
                    let idx = local_idx.min(self.effects.len() - 1);
                    self.effects.remove(idx);
                    return self.emit_update();
                }
            }
            Some("toggle_poly") => {
                self.polyphonic = !self.polyphonic;
                return self.emit_update();
            }
            Some("zero_param") => {
                self.zero_current_param();
                return self.emit_update();
            }
            Some("zero_section") => {
                self.zero_current_section();
                return self.emit_update();
            }
            Some("toggle_lfo") => {
                self.lfo.enabled = !self.lfo.enabled;
                return self.emit_update();
            }
            Some("cycle_lfo_shape") => {
                self.lfo.shape = self.lfo.shape.next();
                return self.emit_update();
            }
            Some("cycle_lfo_target") => {
                self.lfo.target = self.lfo.target.next();
                return self.emit_update();
            }
            Some("next_section") => {
                // Jump to first row of next section
                let current = self.current_section();
                let next = match current {
                    Section::Source => Section::Filter,
                    Section::Filter => Section::Effects,
                    Section::Effects => Section::Lfo,
                    Section::Lfo => Section::Envelope,
                    Section::Envelope => Section::Source,
                };
                for i in 0..self.total_rows() {
                    if self.section_for_row(i) == next {
                        self.selected_row = i;
                        break;
                    }
                }
            }
            Some("prev_section") => {
                // Jump to first row of previous section
                let current = self.current_section();
                let prev = match current {
                    Section::Source => Section::Envelope,
                    Section::Filter => Section::Source,
                    Section::Effects => Section::Filter,
                    Section::Lfo => Section::Effects,
                    Section::Envelope => Section::Lfo,
                };
                for i in 0..self.total_rows() {
                    if self.section_for_row(i) == prev {
                        self.selected_row = i;
                        break;
                    }
                }
            }
            _ => {}
        }
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let rect = center_rect(area, 97, 29);

        let title = format!(" Edit: {} ({}) ", self.instrument_name, self.source.name());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let content_x = inner.x + 1;
        let mut y = inner.y + 1;

        // Mode indicators in header
        let mode_x = rect.x + rect.width - 18;
        let poly_style = ratatui::style::Style::from(Style::new().fg(if self.polyphonic { Color::LIME } else { Color::DARK_GRAY }));
        let poly_str = if self.polyphonic { " POLY " } else { " MONO " };
        Paragraph::new(Line::from(Span::styled(poly_str, poly_style)))
            .render(RatatuiRect::new(mode_x, rect.y, 6, 1), buf);

        // Piano mode indicator
        if self.piano.is_active() {
            let piano_str = self.piano.status_label();
            let piano_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::PINK));
            Paragraph::new(Line::from(Span::styled(piano_str.clone(), piano_style)))
                .render(RatatuiRect::new(rect.x + 1, rect.y, piano_str.len() as u16, 1), buf);
        }

        let mut global_row = 0;

        // === SOURCE SECTION ===
        Paragraph::new(Line::from(Span::styled(
            format!("SOURCE: {}", self.source.name()),
            ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        if self.source_params.is_empty() {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(no parameters)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            global_row += 1;
        } else {
            for param in &self.source_params {
                let is_sel = self.selected_row == global_row;
                render_param_row_buf(buf, content_x, y, param, is_sel, self.editing && is_sel, &self.edit_input);
                y += 1;
                global_row += 1;
            }
        }
        y += 1;

        // === FILTER SECTION ===
        let filter_label = if let Some(ref f) = self.filter {
            format!("FILTER: {}  (f: off, t: cycle)", f.filter_type.name())
        } else {
            "FILTER: OFF  (f: enable)".to_string()
        };
        Paragraph::new(Line::from(Span::styled(
            filter_label,
            ratatui::style::Style::from(Style::new().fg(Color::FILTER_COLOR).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        if let Some(ref f) = self.filter {
            // Type row
            {
                let is_sel = self.selected_row == global_row;
                render_label_value_row_buf(buf, content_x, y, "Type", &f.filter_type.name(), Color::FILTER_COLOR, is_sel);
                y += 1;
                global_row += 1;
            }
            // Cutoff row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row_buf(buf, content_x, y, "Cutoff", f.cutoff.value, f.cutoff.min, f.cutoff.max, is_sel, self.editing && is_sel, &self.edit_input);
                y += 1;
                global_row += 1;
            }
            // Resonance row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row_buf(buf, content_x, y, "Resonance", f.resonance.value, f.resonance.min, f.resonance.max, is_sel, self.editing && is_sel, &self.edit_input);
                y += 1;
                global_row += 1;
            }
        } else {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(disabled)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === EFFECTS SECTION ===
        Paragraph::new(Line::from(Span::styled(
            "EFFECTS  (a: add, d: remove)",
            ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        if self.effects.is_empty() {
            let is_sel = self.selected_row == global_row;
            let style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            Paragraph::new(Line::from(Span::styled("(no effects)", style)))
                .render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);
            global_row += 1;
        } else {
            for effect in &self.effects {
                let is_sel = self.selected_row == global_row;
                // Selection indicator
                if is_sel {
                    if let Some(cell) = buf.cell_mut((content_x, y)) {
                        cell.set_char('>').set_style(
                            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                        );
                    }
                }

                let enabled_str = if effect.enabled { "ON " } else { "OFF" };
                let effect_text = format!("{:10} [{}]", effect.effect_type.name(), enabled_str);
                let effect_style = if is_sel {
                    ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR))
                };
                Paragraph::new(Line::from(Span::styled(effect_text, effect_style)))
                    .render(RatatuiRect::new(content_x + 2, y, 18, 1), buf);

                // Params inline
                let params_str: String = effect.params.iter().take(3).map(|p| {
                    match &p.value {
                        ParamValue::Float(v) => format!("{}:{:.2}", p.name, v),
                        ParamValue::Int(v) => format!("{}:{}", p.name, v),
                        ParamValue::Bool(v) => format!("{}:{}", p.name, v),
                    }
                }).collect::<Vec<_>>().join("  ");
                let params_style = if is_sel {
                    ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
                };
                Paragraph::new(Line::from(Span::styled(params_str, params_style)))
                    .render(RatatuiRect::new(content_x + 20, y, inner.width.saturating_sub(22), 1), buf);

                y += 1;
                global_row += 1;
            }
        }
        y += 1;

        // === LFO SECTION ===
        let lfo_status = if self.lfo.enabled { "ON" } else { "OFF" };
        Paragraph::new(Line::from(Span::styled(
            format!("LFO [{}]  (l: toggle, s: shape, m: target)", lfo_status),
            ratatui::style::Style::from(Style::new().fg(Color::PINK).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        // Row 0: Enabled
        {
            let is_sel = self.selected_row == global_row;
            let enabled_val = if self.lfo.enabled { "ON" } else { "OFF" };
            render_label_value_row_buf(buf, content_x, y, "Enabled", enabled_val, Color::PINK, is_sel);
            y += 1;
            global_row += 1;
        }

        // Row 1: Rate
        {
            let is_sel = self.selected_row == global_row;
            render_value_row_buf(buf, content_x, y, "Rate", self.lfo.rate, 0.1, 32.0, is_sel, self.editing && is_sel, &self.edit_input);
            // Hz label
            let hz_style = if is_sel {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            for (j, ch) in "Hz".chars().enumerate() {
                if let Some(cell) = buf.cell_mut((content_x + 44 + j as u16, y)) {
                    cell.set_char(ch).set_style(hz_style);
                }
            }
            y += 1;
            global_row += 1;
        }

        // Row 2: Depth
        {
            let is_sel = self.selected_row == global_row;
            render_value_row_buf(buf, content_x, y, "Depth", self.lfo.depth, 0.0, 1.0, is_sel, self.editing && is_sel, &self.edit_input);
            y += 1;
            global_row += 1;
        }

        // Row 3: Shape and Target
        {
            let is_sel = self.selected_row == global_row;
            let shape_val = format!("{} → {}", self.lfo.shape.name(), self.lfo.target.name());
            render_label_value_row_buf(buf, content_x, y, "Shape/Dest", &shape_val, Color::PINK, is_sel);
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === ENVELOPE SECTION ===
        Paragraph::new(Line::from(Span::styled(
            "ENVELOPE (ADSR)  (p: poly, r: track)",
            ratatui::style::Style::from(Style::new().fg(Color::ENV_COLOR).bold()),
        ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
        y += 1;

        let env_labels = ["Attack", "Decay", "Sustain", "Release"];
        let env_values = [
            self.amp_envelope.attack,
            self.amp_envelope.decay,
            self.amp_envelope.sustain,
            self.amp_envelope.release,
        ];
        let env_maxes = [5.0, 5.0, 1.0, 5.0];

        for (label, (val, max)) in env_labels.iter().zip(env_values.iter().zip(env_maxes.iter())) {
            let is_sel = self.selected_row == global_row;
            render_value_row_buf(buf, content_x, y, label, *val, 0.0, *max, is_sel, self.editing && is_sel, &self.edit_input);
            y += 1;
            global_row += 1;
        }

        // Suppress unused variable warning
        let _ = global_row;

        // Help text
        let help_y = rect.y + rect.height - 2;
        let help_text = if self.piano.is_active() {
            "Play keys | [/]: octave | \u{2190}/\u{2192}: adjust | \\: zero | /: cycle | Esc: exit"
        } else {
            "\u{2191}/\u{2193}: move | Tab/S-Tab: section | \u{2190}/\u{2192}: adjust | \\: zero | /: piano | Esc: done"
        };
        Paragraph::new(Line::from(Span::styled(
            help_text,
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(RatatuiRect::new(content_x, help_y, inner.width.saturating_sub(2), 1), buf);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn wants_exclusive_input(&self) -> bool {
        self.editing || self.piano.is_active()
    }

    fn toggle_piano_mode(&mut self, _state: &AppState) -> bool {
        if self.piano.is_active() {
            self.piano.handle_escape();
        } else {
            self.piano.activate();
        }
        true
    }

    fn exit_piano_mode(&mut self) -> bool {
        if self.piano.is_active() {
            self.piano.deactivate();
            true
        } else {
            false
        }
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

fn render_param_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    param: &Param,
    is_selected: bool,
    is_editing: bool,
    edit_input: &TextInput,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    // Param name
    let name_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN))
    };
    let name_str = format!("{:12}", param.name);
    for (j, ch) in name_str.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(name_style);
        }
    }

    // Slider
    let (val, min, max) = match &param.value {
        ParamValue::Float(v) => (*v, param.min, param.max),
        ParamValue::Int(v) => (*v as f32, param.min, param.max),
        ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, 0.0, 1.0),
    };
    let slider = render_slider(val, min, max, 16);
    let slider_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::LIME))
    };
    for (j, ch) in slider.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 15 + j as u16, y)) {
            cell.set_char(ch).set_style(slider_style);
        }
    }

    // Value or text input
    if is_editing {
        edit_input.render_buf(buf, x + 34, y, 10);
    } else {
        let value_str = match &param.value {
            ParamValue::Float(v) => format!("{:.2}", v),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => format!("{}", v),
        };
        let val_style = if is_selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE))
        };
        let formatted = format!("{:10}", value_str);
        for (j, ch) in formatted.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + 34 + j as u16, y)) {
                cell.set_char(ch).set_style(val_style);
            }
        }
    }
}

fn render_value_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    name: &str,
    value: f32, min: f32, max: f32,
    is_selected: bool,
    is_editing: bool,
    edit_input: &TextInput,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    // Label
    let name_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::CYAN))
    };
    let name_str = format!("{:12}", name);
    for (j, ch) in name_str.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(name_style);
        }
    }

    // Slider
    let slider = render_slider(value, min, max, 16);
    let slider_style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(Color::LIME))
    };
    for (j, ch) in slider.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 15 + j as u16, y)) {
            cell.set_char(ch).set_style(slider_style);
        }
    }

    // Value or text input
    if is_editing {
        edit_input.render_buf(buf, x + 34, y, 10);
    } else {
        let val_style = if is_selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE))
        };
        let formatted = format!("{:.2}", value);
        for (j, ch) in formatted.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + 34 + j as u16, y)) {
                cell.set_char(ch).set_style(val_style);
            }
        }
    }
}

/// Render a label-value row (no slider, for type/enabled/shape rows)
fn render_label_value_row_buf(
    buf: &mut Buffer,
    x: u16, y: u16,
    label: &str,
    value: &str,
    color: Color,
    is_selected: bool,
) {
    // Selection indicator
    if is_selected {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char('>').set_style(
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
            );
        }
    }

    let text = format!("{:12}  {}", label, value);
    let style = if is_selected {
        ratatui::style::Style::from(Style::new().fg(color).bg(Color::SELECTION_BG))
    } else {
        ratatui::style::Style::from(Style::new().fg(color))
    };
    for (j, ch) in text.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x + 2 + j as u16, y)) {
            cell.set_char(ch).set_style(style);
        }
    }
}
