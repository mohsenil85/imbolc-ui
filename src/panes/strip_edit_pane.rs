use std::any::Any;

use crate::state::{
    EffectSlot, EffectType, EnvConfig, FilterConfig, FilterType, OscType, Param, ParamValue,
    StripId, Strip,
};
use crate::ui::widgets::TextInput;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Source,
    Filter,
    Effects,
    Envelope,
}

impl Tab {
    fn all() -> &'static [Tab] {
        &[Tab::Source, Tab::Filter, Tab::Effects, Tab::Envelope]
    }

    fn name(&self) -> &'static str {
        match self {
            Tab::Source => "Source",
            Tab::Filter => "Filter",
            Tab::Effects => "Effects",
            Tab::Envelope => "Envelope",
        }
    }

    fn next(&self) -> Tab {
        match self {
            Tab::Source => Tab::Filter,
            Tab::Filter => Tab::Effects,
            Tab::Effects => Tab::Envelope,
            Tab::Envelope => Tab::Source,
        }
    }
}

pub struct StripEditPane {
    keymap: Keymap,
    strip_id: Option<StripId>,
    strip_name: String,
    source: OscType,
    source_params: Vec<Param>,
    filter: Option<FilterConfig>,
    effects: Vec<EffectSlot>,
    amp_envelope: EnvConfig,
    polyphonic: bool,
    has_track: bool,
    tab: Tab,
    selected_row: usize,
    editing: bool,
    edit_input: TextInput,
}

impl StripEditPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Escape, "done", "Done editing")
                .bind_key(KeyCode::Tab, "next_tab", "Next tab")
                .bind_key(KeyCode::Down, "next", "Next item")
                .bind_key(KeyCode::Up, "prev", "Previous item")
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
                .bind('r', "toggle_track", "Toggle piano roll track"),
            strip_id: None,
            strip_name: String::new(),
            source: OscType::Saw,
            source_params: Vec::new(),
            filter: None,
            effects: Vec::new(),
            amp_envelope: EnvConfig::default(),
            polyphonic: true,
            has_track: true,
            tab: Tab::Source,
            selected_row: 0,
            editing: false,
            edit_input: TextInput::new(""),
        }
    }

    pub fn set_strip(&mut self, strip: &Strip) {
        self.strip_id = Some(strip.id);
        self.strip_name = strip.name.clone();
        self.source = strip.source;
        self.source_params = strip.source_params.clone();
        self.filter = strip.filter.clone();
        self.effects = strip.effects.clone();
        self.amp_envelope = strip.amp_envelope.clone();
        self.polyphonic = strip.polyphonic;
        self.has_track = strip.has_track;
        self.tab = Tab::Source;
        self.selected_row = 0;
    }

    pub fn strip_id(&self) -> Option<StripId> {
        self.strip_id
    }

    /// Apply edits back to a strip
    pub fn apply_to(&self, strip: &mut Strip) {
        strip.source = self.source;
        strip.source_params = self.source_params.clone();
        strip.filter = self.filter.clone();
        strip.effects = self.effects.clone();
        strip.amp_envelope = self.amp_envelope.clone();
        strip.polyphonic = self.polyphonic;
        strip.has_track = self.has_track;
    }

    fn current_params(&self) -> Vec<&Param> {
        match self.tab {
            Tab::Source => self.source_params.iter().collect(),
            Tab::Filter => {
                if let Some(ref _f) = self.filter {
                    // Present cutoff and resonance as pseudo-params
                    vec![]
                } else {
                    vec![]
                }
            }
            Tab::Effects => vec![],
            Tab::Envelope => vec![],
        }
    }

    fn row_count(&self) -> usize {
        match self.tab {
            Tab::Source => self.source_params.len(),
            Tab::Filter => if self.filter.is_some() { 3 } else { 1 }, // type, cutoff, resonance (or just "off")
            Tab::Effects => self.effects.len().max(1), // at least one row for "add"
            Tab::Envelope => 4, // A, D, S, R
        }
    }

    fn adjust_value(&mut self, increase: bool, big: bool) {
        let fraction = if big { 0.10 } else { 0.05 };
        match self.tab {
            Tab::Source => {
                if let Some(param) = self.source_params.get_mut(self.selected_row) {
                    adjust_param(param, increase, fraction);
                }
            }
            Tab::Filter => {
                if let Some(ref mut f) = self.filter {
                    match self.selected_row {
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
            Tab::Effects => {
                if let Some(effect) = self.effects.get_mut(self.selected_row) {
                    // For effects, we'd need a sub-selection for params
                    // For now, adjust the first param
                    if let Some(param) = effect.params.first_mut() {
                        adjust_param(param, increase, fraction);
                    }
                }
            }
            Tab::Envelope => {
                let delta = if big { 0.1 } else { 0.05 };
                let val = match self.selected_row {
                    0 => &mut self.amp_envelope.attack,
                    1 => &mut self.amp_envelope.decay,
                    2 => &mut self.amp_envelope.sustain,
                    3 => &mut self.amp_envelope.release,
                    _ => return,
                };
                if increase { *val = (*val + delta).min(if self.selected_row == 2 { 1.0 } else { 5.0 }); }
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

fn render_slider(value: f32, min: f32, max: f32) -> String {
    const W: usize = 20;
    let normalized = (value - min) / (max - min);
    let pos = (normalized * W as f32) as usize;
    let pos = pos.min(W);
    let mut s = String::with_capacity(W + 2);
    s.push('[');
    for i in 0..W {
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
        if self.editing {
            match event.key {
                KeyCode::Enter => {
                    let text = self.edit_input.value().to_string();
                    // Apply text edit based on tab/row
                    match self.tab {
                        Tab::Source => {
                            if let Some(param) = self.source_params.get_mut(self.selected_row) {
                                if let Ok(v) = text.parse::<f32>() {
                                    param.value = ParamValue::Float(v.clamp(param.min, param.max));
                                }
                            }
                        }
                        Tab::Filter => {
                            if let Some(ref mut f) = self.filter {
                                match self.selected_row {
                                    1 => if let Ok(v) = text.parse::<f32>() { f.cutoff.value = v.clamp(f.cutoff.min, f.cutoff.max); },
                                    2 => if let Ok(v) = text.parse::<f32>() { f.resonance.value = v.clamp(f.resonance.min, f.resonance.max); },
                                    _ => {}
                                }
                            }
                        }
                        Tab::Envelope => {
                            if let Ok(v) = text.parse::<f32>() {
                                let max = if self.selected_row == 2 { 1.0 } else { 5.0 };
                                let val = v.clamp(0.0, max);
                                match self.selected_row {
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
            Some("next_tab") => {
                self.tab = self.tab.next();
                self.selected_row = 0;
            }
            Some("next") => {
                let count = self.row_count();
                if count > 0 {
                    self.selected_row = (self.selected_row + 1) % count;
                }
            }
            Some("prev") => {
                let count = self.row_count();
                if count > 0 {
                    self.selected_row = if self.selected_row == 0 { count - 1 } else { self.selected_row - 1 };
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
                self.selected_row = 0;
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
                if self.tab == Tab::Effects {
                    // Cycle through available effect types
                    let next_type = if self.effects.is_empty() {
                        EffectType::Delay
                    } else {
                        match self.effects.last().unwrap().effect_type {
                            EffectType::Delay => EffectType::Reverb,
                            EffectType::Reverb => EffectType::Delay,
                        }
                    };
                    self.effects.push(EffectSlot::new(next_type));
                    return self.emit_update();
                }
            }
            Some("remove_effect") => {
                if self.tab == Tab::Effects && !self.effects.is_empty() {
                    let idx = self.selected_row.min(self.effects.len() - 1);
                    self.effects.remove(idx);
                    if self.selected_row > 0 && self.selected_row >= self.effects.len() {
                        self.selected_row = self.effects.len().saturating_sub(1);
                    }
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
        let content_y = rect.y + 2;

        // Tab bar
        let mut tab_x = content_x;
        for tab in Tab::all() {
            let is_active = *tab == self.tab;
            if is_active {
                g.set_style(Style::new().fg(Color::ORANGE).bold());
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
            }
            let label = format!(" {} ", tab.name());
            g.put_str(tab_x, content_y, &label);
            tab_x += label.len() as u16 + 1;
        }

        // Mode indicators
        let mode_x = rect.x + rect.width - 20;
        g.set_style(Style::new().fg(if self.polyphonic { Color::LIME } else { Color::DARK_GRAY }));
        g.put_str(mode_x, content_y, if self.polyphonic { "POLY" } else { "MONO" });
        g.set_style(Style::new().fg(if self.has_track { Color::PINK } else { Color::DARK_GRAY }));
        g.put_str(mode_x + 6, content_y, if self.has_track { "TRK" } else { "---" });

        let list_y = content_y + 2;

        match self.tab {
            Tab::Source => {
                g.set_style(Style::new().fg(Color::CYAN).bold());
                g.put_str(content_x, list_y - 1, &format!("Oscillator: {}", self.source.name()));

                for (i, param) in self.source_params.iter().enumerate() {
                    let y = list_y + i as u16 + 1;
                    let is_selected = i == self.selected_row;
                    render_param_row(g, content_x, y, rect.x + rect.width, param, is_selected, self.editing && is_selected, &self.edit_input);
                }
            }
            Tab::Filter => {
                if let Some(ref f) = self.filter {
                    g.set_style(Style::new().fg(Color::FILTER_COLOR).bold());
                    g.put_str(content_x, list_y - 1, &format!("Filter: {}", f.filter_type.name()));

                    // Row 0: type
                    {
                        let y = list_y + 1;
                        let is_selected = self.selected_row == 0;
                        if is_selected {
                            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                            g.put_str(content_x, y, ">");
                        } else {
                            g.put_str(content_x, y, " ");
                        }
                        if is_selected {
                            g.set_style(Style::new().fg(Color::FILTER_COLOR).bg(Color::SELECTION_BG));
                        } else {
                            g.set_style(Style::new().fg(Color::FILTER_COLOR));
                        }
                        g.put_str(content_x + 2, y, &format!("{:12}  {}", "Type", f.filter_type.name()));
                    }

                    // Row 1: cutoff
                    {
                        let y = list_y + 2;
                        let is_selected = self.selected_row == 1;
                        render_modulated_row(g, content_x, y, rect.x + rect.width, "Cutoff", f.cutoff.value, f.cutoff.min, f.cutoff.max, is_selected, self.editing && is_selected, &self.edit_input);
                    }

                    // Row 2: resonance
                    {
                        let y = list_y + 3;
                        let is_selected = self.selected_row == 2;
                        render_modulated_row(g, content_x, y, rect.x + rect.width, "Resonance", f.resonance.value, f.resonance.min, f.resonance.max, is_selected, self.editing && is_selected, &self.edit_input);
                    }
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, list_y, "Filter: OFF  (press 'f' to enable)");
                }
            }
            Tab::Effects => {
                g.set_style(Style::new().fg(Color::FX_COLOR).bold());
                g.put_str(content_x, list_y - 1, "Effects Chain:");

                if self.effects.is_empty() {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x + 2, list_y + 1, "(no effects — press 'a' to add)");
                } else {
                    for (i, effect) in self.effects.iter().enumerate() {
                        let y = list_y + 1 + i as u16;
                        let is_selected = i == self.selected_row;

                        if is_selected {
                            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                            g.put_str(content_x, y, ">");
                        } else {
                            g.set_style(Style::new().fg(Color::DARK_GRAY));
                            g.put_str(content_x, y, " ");
                        }

                        let enabled_str = if effect.enabled { "ON " } else { "OFF" };
                        if is_selected {
                            g.set_style(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG));
                        } else {
                            g.set_style(Style::new().fg(Color::FX_COLOR));
                        }
                        g.put_str(content_x + 2, y, &format!("{:10} [{}]", effect.effect_type.name(), enabled_str));

                        // Show params inline
                        let params_str: String = effect.params.iter().take(3).map(|p| {
                            match &p.value {
                                ParamValue::Float(v) => format!("{}:{:.1}", p.name, v),
                                ParamValue::Int(v) => format!("{}:{}", p.name, v),
                                ParamValue::Bool(v) => format!("{}:{}", p.name, v),
                            }
                        }).collect::<Vec<_>>().join("  ");
                        if is_selected {
                            g.set_style(Style::new().fg(Color::SKY_BLUE).bg(Color::SELECTION_BG));
                        } else {
                            g.set_style(Style::new().fg(Color::DARK_GRAY));
                        }
                        g.put_str(content_x + 20, y, &params_str);
                    }
                }
            }
            Tab::Envelope => {
                g.set_style(Style::new().fg(Color::ENV_COLOR).bold());
                g.put_str(content_x, list_y - 1, "Amplitude Envelope (ADSR):");

                let labels = ["Attack", "Decay", "Sustain", "Release"];
                let values = [
                    self.amp_envelope.attack,
                    self.amp_envelope.decay,
                    self.amp_envelope.sustain,
                    self.amp_envelope.release,
                ];
                let maxes = [5.0, 5.0, 1.0, 5.0];

                for (i, (label, (val, max))) in labels.iter().zip(values.iter().zip(maxes.iter())).enumerate() {
                    let y = list_y + 1 + i as u16;
                    let is_selected = i == self.selected_row;

                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                        g.put_str(content_x, y, ">");
                    } else {
                        g.set_style(Style::new().fg(Color::DARK_GRAY));
                        g.put_str(content_x, y, " ");
                    }

                    if is_selected {
                        g.set_style(Style::new().fg(Color::CYAN).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(Color::CYAN));
                    }
                    g.put_str(content_x + 2, y, &format!("{:12}", label));

                    let slider = render_slider(*val, 0.0, *max);
                    if is_selected {
                        g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(Color::LIME));
                    }
                    g.put_str(content_x + 15, y, &slider);

                    if is_selected && self.editing {
                        self.edit_input.render(g, content_x + 38, y, 12);
                    } else {
                        if is_selected {
                            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                        } else {
                            g.set_style(Style::new().fg(Color::WHITE));
                        }
                        g.put_str(content_x + 38, y, &format!("{:.3}", val));
                    }
                }
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        let help_text = match self.tab {
            Tab::Source => "←/→: adjust | ↑/↓: select | Enter: type | Tab: next tab | Esc: done",
            Tab::Filter => "f: toggle | t: cycle type | ←/→: adjust | Tab: next tab | Esc: done",
            Tab::Effects => "a: add | d: remove | ←/→: adjust | Tab: next tab | Esc: done",
            Tab::Envelope => "←/→: adjust | Enter: type value | p: poly | Tab: next tab | Esc: done",
        };
        g.put_str(content_x, help_y, help_text);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
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
    x: u16, y: u16, right_edge: u16,
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
    let slider = render_slider(val, min, max);
    if is_selected {
        g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::LIME));
    }
    g.put_str(x + 15, y, &slider);

    if is_editing {
        edit_input.render(g, x + 38, y, 12);
    } else {
        let value_str = match &param.value {
            ParamValue::Float(v) => format!("{:.1}", v),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => format!("{}", v),
        };
        if is_selected {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
        } else {
            g.set_style(Style::new().fg(Color::WHITE));
        }
        g.put_str(x + 38, y, &format!("{:10}", value_str));
    }

    if is_selected {
        g.set_style(Style::new().bg(Color::SELECTION_BG));
        let line_end = x + 50;
        for cx in line_end..(right_edge - 2) {
            g.put_char(cx, y, ' ');
        }
    }
}

fn render_modulated_row(
    g: &mut dyn Graphics,
    x: u16, y: u16, right_edge: u16,
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

    let slider = render_slider(value, min, max);
    if is_selected {
        g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
    } else {
        g.set_style(Style::new().fg(Color::LIME));
    }
    g.put_str(x + 15, y, &slider);

    if is_editing {
        edit_input.render(g, x + 38, y, 12);
    } else {
        if is_selected {
            g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
        } else {
            g.set_style(Style::new().fg(Color::WHITE));
        }
        g.put_str(x + 38, y, &format!("{:.1}", value));
    }

    if is_selected {
        g.set_style(Style::new().bg(Color::SELECTION_BG));
        let line_end = x + 50;
        for cx in line_end..(right_edge - 2) {
            g.put_char(cx, y, ' ');
        }
    }
}
