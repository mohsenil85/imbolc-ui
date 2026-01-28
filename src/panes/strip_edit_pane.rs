use std::any::Any;

use crate::state::{
    EffectSlot, EffectType, EnvConfig, FilterConfig, FilterType, LfoConfig, LfoShape, LfoTarget,
    OscType, Param, ParamValue, StripId, Strip,
};
use crate::ui::widgets::TextInput;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

/// Which section a row belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Source,
    Filter,
    Effects,
    Lfo,
    Envelope,
}

/// Piano keyboard layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PianoLayout {
    C,
    A,
}

pub struct StripEditPane {
    keymap: Keymap,
    strip_id: Option<StripId>,
    strip_name: String,
    source: OscType,
    source_params: Vec<Param>,
    filter: Option<FilterConfig>,
    effects: Vec<EffectSlot>,
    lfo: LfoConfig,
    amp_envelope: EnvConfig,
    polyphonic: bool,
    has_track: bool,
    selected_row: usize,
    editing: bool,
    edit_input: TextInput,
    // Piano mode
    piano_mode: bool,
    piano_octave: i8,
    piano_layout: PianoLayout,
}

impl StripEditPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "done", "Done editing")
                .bind('/', "piano_mode", "Toggle piano keyboard")
                .bind_key(KeyCode::Down, "next", "Next item")
                .bind_key(KeyCode::Up, "prev", "Previous item")
                .bind_key(KeyCode::Tab, "next_section", "Next section")
                .bind_key(KeyCode::Left, "decrease", "Decrease value")
                .bind_key(KeyCode::Right, "increase", "Increase value")
                .bind_key(KeyCode::PageUp, "increase_big", "Increase +10%")
                .bind_key(KeyCode::PageDown, "decrease_big", "Decrease -10%")
                .bind_key(KeyCode::Enter, "enter_edit", "Type value")
                .bind('f', "toggle_filter", "Toggle filter on/off")
                .bind('t', "cycle_filter_type", "Cycle filter type")
                .bind('a', "add_effect", "Add effect")
                .bind('d', "remove_effect", "Remove effect")
                .bind('p', "toggle_poly", "Toggle polyphonic")
                .bind('r', "toggle_track", "Toggle piano roll track")
                .bind('\\', "zero_param", "Set param to zero")
                .bind('|', "zero_section", "Zero all params in section")
                .bind('l', "toggle_lfo", "Toggle LFO on/off")
                .bind('s', "cycle_lfo_shape", "Cycle LFO shape")
                .bind('m', "cycle_lfo_target", "Cycle LFO target"),
            strip_id: None,
            strip_name: String::new(),
            source: OscType::Saw,
            source_params: Vec::new(),
            filter: None,
            effects: Vec::new(),
            lfo: LfoConfig::default(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            has_track: true,
            selected_row: 0,
            editing: false,
            edit_input: TextInput::new(""),
            piano_mode: false,
            piano_octave: 4,
            piano_layout: PianoLayout::C,
        }
    }

    pub fn set_strip(&mut self, strip: &Strip) {
        self.strip_id = Some(strip.id);
        self.strip_name = strip.name.clone();
        self.source = strip.source;
        self.source_params = strip.source_params.clone();
        self.filter = strip.filter.clone();
        self.effects = strip.effects.clone();
        self.lfo = strip.lfo.clone();
        self.amp_envelope = strip.amp_envelope.clone();
        self.polyphonic = strip.polyphonic;
        self.has_track = strip.has_track;
        self.selected_row = 0;
    }

    pub fn strip_id(&self) -> Option<StripId> {
        self.strip_id
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

    /// Apply edits back to a strip
    pub fn apply_to(&self, strip: &mut Strip) {
        strip.source = self.source;
        strip.source_params = self.source_params.clone();
        strip.filter = self.filter.clone();
        strip.effects = self.effects.clone();
        strip.lfo = self.lfo.clone();
        strip.amp_envelope = self.amp_envelope.clone();
        strip.polyphonic = self.polyphonic;
        strip.has_track = self.has_track;
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
        if let Some(id) = self.strip_id {
            Action::UpdateStrip(id)
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

    // Piano key mapping
    fn key_to_offset_c(key: char) -> Option<u8> {
        match key {
            'a' => Some(0), 's' => Some(2), 'd' => Some(4), 'f' => Some(5),
            'g' => Some(7), 'h' => Some(9), 'j' => Some(11),
            'w' => Some(1), 'e' => Some(3), 't' => Some(6), 'y' => Some(8), 'u' => Some(10),
            'k' => Some(12), 'l' => Some(14), ';' => Some(16),
            'o' => Some(13), 'p' => Some(15),
            _ => None,
        }
    }

    fn key_to_offset_a(key: char) -> Option<u8> {
        match key {
            'a' => Some(0), 's' => Some(2), 'd' => Some(3), 'f' => Some(5),
            'g' => Some(7), 'h' => Some(8), 'j' => Some(10),
            'w' => Some(1), 'e' => Some(4), 't' => Some(6), 'y' => Some(9), 'u' => Some(11),
            'k' => Some(12), 'l' => Some(14), ';' => Some(15),
            'o' => Some(13), 'p' => Some(16),
            _ => None,
        }
    }

    fn key_to_pitch(&self, key: char) -> Option<u8> {
        let offset = match self.piano_layout {
            PianoLayout::C => Self::key_to_offset_c(key),
            PianoLayout::A => Self::key_to_offset_a(key),
        };
        offset.map(|off| {
            let base = match self.piano_layout {
                PianoLayout::C => (self.piano_octave as i16 + 1) * 12,
                PianoLayout::A => (self.piano_octave as i16 + 1) * 12 - 3,
            };
            (base + off as i16).clamp(0, 127) as u8
        })
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

impl Pane for StripEditPane {
    fn id(&self) -> &'static str {
        "strip_edit"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        // Handle Shift+Tab for prev_section (before other handlers)
        if event.key == KeyCode::Tab && event.modifiers.shift {
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
            return Action::None;
        }

        // Piano mode
        if self.piano_mode {
            match event.key {
                KeyCode::Char('/') => {
                    match self.piano_layout {
                        PianoLayout::C => self.piano_layout = PianoLayout::A,
                        PianoLayout::A => self.piano_mode = false,
                    }
                    return Action::None;
                }
                KeyCode::Char('[') => {
                    if self.piano_octave > -1 { self.piano_octave -= 1; }
                    return Action::None;
                }
                KeyCode::Char(']') => {
                    if self.piano_octave < 9 { self.piano_octave += 1; }
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
                KeyCode::Escape => {
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
                    if let Some(pitch) = self.key_to_pitch(c) {
                        let velocity = if event.modifiers.shift { 127 } else { 100 };
                        return Action::StripPlayNote(pitch, velocity);
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
            Some("piano_mode") => {
                self.piano_mode = true;
                self.piano_layout = PianoLayout::C;
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
            Some("toggle_track") => {
                self.has_track = !self.has_track;
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

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        let title = format!(" Edit: {} ({}) ", self.strip_name, self.source.name());
        g.set_style(Style::new().fg(Color::ORANGE));
        g.draw_box(rect, Some(&title));

        let content_x = rect.x + 2;
        let mut y = rect.y + 2;

        // Mode indicators in header
        let mode_x = rect.x + rect.width - 18;
        g.set_style(Style::new().fg(if self.polyphonic { Color::LIME } else { Color::DARK_GRAY }));
        g.put_str(mode_x, rect.y, if self.polyphonic { " POLY " } else { " MONO " });
        g.set_style(Style::new().fg(if self.has_track { Color::PINK } else { Color::DARK_GRAY }));
        g.put_str(mode_x + 7, rect.y, if self.has_track { " TRK " } else { " --- " });

        // Piano mode indicator
        if self.piano_mode {
            g.set_style(Style::new().fg(Color::BLACK).bg(Color::PINK));
            let layout_char = match self.piano_layout { PianoLayout::C => 'C', PianoLayout::A => 'A' };
            let piano_str = format!(" PIANO {}{} ", layout_char, self.piano_octave);
            g.put_str(rect.x + 1, rect.y, &piano_str);
        }

        let mut global_row = 0;

        // === SOURCE SECTION ===
        g.set_style(Style::new().fg(Color::CYAN).bold());
        g.put_str(content_x, y, &format!("SOURCE: {}", self.source.name()));
        y += 1;

        if self.source_params.is_empty() {
            let is_sel = self.selected_row == global_row;
            if is_sel {
                g.set_style(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            g.put_str(content_x + 2, y, "(no parameters)");
            global_row += 1;
        } else {
            for param in &self.source_params {
                let is_sel = self.selected_row == global_row;
                render_param_row(g, content_x, y, param, is_sel, self.editing && is_sel, &self.edit_input);
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
        g.set_style(Style::new().fg(Color::FILTER_COLOR).bold());
        g.put_str(content_x, y, &filter_label);
        y += 1;

        if let Some(ref f) = self.filter {
            // Type row
            {
                let is_sel = self.selected_row == global_row;
                if is_sel {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                    g.put_str(content_x, y, ">");
                    g.set_style(Style::new().fg(Color::FILTER_COLOR).bg(Color::SELECTION_BG));
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, y, " ");
                    g.set_style(Style::new().fg(Color::FILTER_COLOR));
                }
                g.put_str(content_x + 2, y, &format!("{:12}  {}", "Type", f.filter_type.name()));
                y += 1;
                global_row += 1;
            }
            // Cutoff row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row(g, content_x, y, "Cutoff", f.cutoff.value, f.cutoff.min, f.cutoff.max, is_sel, self.editing && is_sel, &self.edit_input);
                y += 1;
                global_row += 1;
            }
            // Resonance row
            {
                let is_sel = self.selected_row == global_row;
                render_value_row(g, content_x, y, "Resonance", f.resonance.value, f.resonance.min, f.resonance.max, is_sel, self.editing && is_sel, &self.edit_input);
                y += 1;
                global_row += 1;
            }
        } else {
            let is_sel = self.selected_row == global_row;
            if is_sel {
                g.set_style(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            g.put_str(content_x + 2, y, "(disabled)");
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === EFFECTS SECTION ===
        g.set_style(Style::new().fg(Color::FX_COLOR).bold());
        g.put_str(content_x, y, "EFFECTS  (a: add, d: remove)");
        y += 1;

        if self.effects.is_empty() {
            let is_sel = self.selected_row == global_row;
            if is_sel {
                g.set_style(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            g.put_str(content_x + 2, y, "(no effects)");
            global_row += 1;
        } else {
            for effect in &self.effects {
                let is_sel = self.selected_row == global_row;
                if is_sel {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                    g.put_str(content_x, y, ">");
                    g.set_style(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG));
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, y, " ");
                    g.set_style(Style::new().fg(Color::FX_COLOR));
                }
                let enabled_str = if effect.enabled { "ON " } else { "OFF" };
                g.put_str(content_x + 2, y, &format!("{:10} [{}]", effect.effect_type.name(), enabled_str));

                // Show first few params inline
                let params_str: String = effect.params.iter().take(3).map(|p| {
                    match &p.value {
                        ParamValue::Float(v) => format!("{}:{:.2}", p.name, v),
                        ParamValue::Int(v) => format!("{}:{}", p.name, v),
                        ParamValue::Bool(v) => format!("{}:{}", p.name, v),
                    }
                }).collect::<Vec<_>>().join("  ");
                if is_sel {
                    g.set_style(Style::new().fg(Color::SKY_BLUE).bg(Color::SELECTION_BG));
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                }
                g.put_str(content_x + 20, y, &params_str);

                y += 1;
                global_row += 1;
            }
        }
        y += 1;

        // === LFO SECTION ===
        let lfo_status = if self.lfo.enabled { "ON" } else { "OFF" };
        g.set_style(Style::new().fg(Color::PINK).bold());
        g.put_str(content_x, y, &format!("LFO [{}]  (l: toggle, s: shape, m: target)", lfo_status));
        y += 1;

        // Row 0: Enabled
        {
            let is_sel = self.selected_row == global_row;
            if is_sel {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, ">");
                g.set_style(Style::new().fg(Color::PINK).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x, y, " ");
                g.set_style(Style::new().fg(Color::PINK));
            }
            g.put_str(content_x + 2, y, &format!("{:12}  {}", "Enabled", if self.lfo.enabled { "ON" } else { "OFF" }));
            y += 1;
            global_row += 1;
        }

        // Row 1: Rate
        {
            let is_sel = self.selected_row == global_row;
            render_value_row(g, content_x, y, "Rate", self.lfo.rate, 0.1, 32.0, is_sel, self.editing && is_sel, &self.edit_input);
            // Add Hz label
            if is_sel {
                g.set_style(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            g.put_str(content_x + 44, y, "Hz");
            y += 1;
            global_row += 1;
        }

        // Row 2: Depth
        {
            let is_sel = self.selected_row == global_row;
            render_value_row(g, content_x, y, "Depth", self.lfo.depth, 0.0, 1.0, is_sel, self.editing && is_sel, &self.edit_input);
            y += 1;
            global_row += 1;
        }

        // Row 3: Shape and Target
        {
            let is_sel = self.selected_row == global_row;
            if is_sel {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, ">");
                g.set_style(Style::new().fg(Color::PINK).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x, y, " ");
                g.set_style(Style::new().fg(Color::PINK));
            }
            g.put_str(content_x + 2, y, &format!("{:12}  {} → {}", "Shape/Dest", self.lfo.shape.name(), self.lfo.target.name()));
            y += 1;
            global_row += 1;
        }
        y += 1;

        // === ENVELOPE SECTION ===
        g.set_style(Style::new().fg(Color::ENV_COLOR).bold());
        g.put_str(content_x, y, "ENVELOPE (ADSR)  (p: poly, r: track)");
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
            render_value_row(g, content_x, y, label, *val, 0.0, *max, is_sel, self.editing && is_sel, &self.edit_input);
            y += 1;
            global_row += 1;
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        if self.piano_mode {
            g.put_str(content_x, help_y, "Play keys | [/]: octave | ←/→: adjust | \\: zero | /: cycle/exit");
        } else {
            g.put_str(content_x, help_y, "↑/↓: move | Tab/S-Tab: section | ←/→: adjust | \\: zero | /: piano | Esc: done");
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn wants_exclusive_input(&self) -> bool {
        self.editing || self.piano_mode
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Default for StripEditPane {
    fn default() -> Self {
        Self::new()
    }
}

fn render_param_row(
    g: &mut dyn Graphics,
    x: u16, y: u16,
    param: &Param,
    is_selected: bool,
    is_editing: bool,
    edit_input: &TextInput,
) {
    if is_selected {
        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
        g.put_str(x, y, ">");
    } else {
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(x, y, " ");
    }

    if is_selected {
        g.set_style(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::CYAN));
    }
    g.put_str(x + 2, y, &format!("{:12}", param.name));

    let (val, min, max) = match &param.value {
        ParamValue::Float(v) => (*v, param.min, param.max),
        ParamValue::Int(v) => (*v as f32, param.min, param.max),
        ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, 0.0, 1.0),
    };
    let slider = render_slider(val, min, max, 16);
    if is_selected {
        g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::LIME));
    }
    g.put_str(x + 15, y, &slider);

    if is_editing {
        edit_input.render(g, x + 34, y, 10);
    } else {
        let value_str = match &param.value {
            ParamValue::Float(v) => format!("{:.2}", v),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => format!("{}", v),
        };
        if is_selected {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
        } else {
            g.set_style(Style::new().fg(Color::WHITE));
        }
        g.put_str(x + 34, y, &format!("{:10}", value_str));
    }
}

fn render_value_row(
    g: &mut dyn Graphics,
    x: u16, y: u16,
    name: &str,
    value: f32, min: f32, max: f32,
    is_selected: bool,
    is_editing: bool,
    edit_input: &TextInput,
) {
    if is_selected {
        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
        g.put_str(x, y, ">");
    } else {
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(x, y, " ");
    }

    if is_selected {
        g.set_style(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::CYAN));
    }
    g.put_str(x + 2, y, &format!("{:12}", name));

    let slider = render_slider(value, min, max, 16);
    if is_selected {
        g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::LIME));
    }
    g.put_str(x + 15, y, &slider);

    if is_editing {
        edit_input.render(g, x + 34, y, 10);
    } else {
        if is_selected {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
        } else {
            g.set_style(Style::new().fg(Color::WHITE));
        }
        g.put_str(x + 34, y, &format!("{:.2}", value));
    }
}
