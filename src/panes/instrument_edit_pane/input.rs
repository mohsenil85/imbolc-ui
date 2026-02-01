use super::{InstrumentEditPane, Section};
use crate::state::{
    AppState, EffectSlot, EffectType, FilterConfig, FilterType, ParamValue,
};
use crate::ui::{Action, FileSelectAction, InputEvent, KeyCode, InstrumentAction, SessionAction, translate_key};

impl InstrumentEditPane {
    pub(super) fn handle_action_impl(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
        match action {
            // Piano mode actions
            "piano:escape" => {
                let was_active = self.piano.is_active();
                self.piano.handle_escape();
                if was_active && !self.piano.is_active() {
                    Action::ExitPerformanceMode
                } else {
                    Action::None
                }
            }
            "piano:octave_down" => { self.piano.octave_down(); Action::None }
            "piano:octave_up" => { self.piano.octave_up(); Action::None }
            "piano:key" | "piano:space" => {
                if let KeyCode::Char(c) = event.key {
                    let c = translate_key(c, state.keyboard_layout);
                    if let Some(pitches) = self.piano.key_to_pitches(c) {
                        if pitches.len() == 1 {
                            return Action::Instrument(InstrumentAction::PlayNote(pitches[0], 100));
                        } else {
                            return Action::Instrument(InstrumentAction::PlayNotes(pitches.clone(), 100));
                        }
                    }
                }
                Action::None
            }
            // Text edit layer actions
            "text:confirm" => {
                let text = self.edit_input.value().to_string();
                let (section, local_idx) = self.row_info(self.selected_row);
                match section {
                    Section::Source => {
                        let param_idx = if self.source.is_sample() {
                            if local_idx == 0 {
                                self.editing = false;
                                self.edit_input.set_focused(false);
                                return Action::None;
                            }
                            local_idx - 1
                        } else {
                            local_idx
                        };
                        if let Some(param) = self.source_params.get_mut(param_idx) {
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
                self.emit_update()
            }
            "text:cancel" => {
                self.editing = false;
                self.edit_input.set_focused(false);
                Action::None
            }
            // Normal pane actions
            "done" => {
                self.emit_update()
            }
            "next" => {
                let total = self.total_rows();
                if total > 0 {
                    self.selected_row = (self.selected_row + 1) % total;
                }
                Action::None
            }
            "prev" => {
                let total = self.total_rows();
                if total > 0 {
                    self.selected_row = if self.selected_row == 0 { total - 1 } else { self.selected_row - 1 };
                }
                Action::None
            }
            "increase" => {
                self.adjust_value(true, false);
                self.emit_update()
            }
            "decrease" => {
                self.adjust_value(false, false);
                self.emit_update()
            }
            "increase_big" => {
                self.adjust_value(true, true);
                self.emit_update()
            }
            "decrease_big" => {
                self.adjust_value(false, true);
                self.emit_update()
            }
            "enter_edit" => {
                // On the sample row, trigger load_sample instead of text edit
                if self.source.is_sample() {
                    let (section, local_idx) = self.row_info(self.selected_row);
                    if section == Section::Source && local_idx == 0 {
                        if let Some(id) = self.instrument_id {
                            return Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::LoadPitchedSample(id)));
                        }
                        return Action::None;
                    }
                }
                self.editing = true;
                let current_val = self.current_value_string();
                self.edit_input.set_value(&current_val);
                self.edit_input.set_focused(true);
                Action::PushLayer("text_edit")
            }
            "toggle_filter" => {
                if self.filter.is_some() {
                    self.filter = None;
                } else {
                    self.filter = Some(FilterConfig::new(FilterType::Lpf));
                }
                self.emit_update()
            }
            "cycle_filter_type" => {
                if let Some(ref mut f) = self.filter {
                    f.filter_type = match f.filter_type {
                        FilterType::Lpf => FilterType::Hpf,
                        FilterType::Hpf => FilterType::Bpf,
                        FilterType::Bpf => FilterType::Lpf,
                    };
                    return self.emit_update();
                }
                Action::None
            }
            "add_effect" => {
                let next_type = if self.effects.is_empty() {
                    EffectType::Delay
                } else {
                    match self.effects.last().unwrap().effect_type {
                        EffectType::Delay => EffectType::Reverb,
                        EffectType::Reverb => EffectType::Gate,
                        EffectType::Gate => EffectType::TapeComp,
                        EffectType::TapeComp => EffectType::SidechainComp,
                        EffectType::SidechainComp | EffectType::Vst(_) => EffectType::Delay,
                    }
                };
                self.effects.push(EffectSlot::new(next_type));
                self.emit_update()
            }
            "remove_effect" => {
                let (section, local_idx) = self.row_info(self.selected_row);
                if section == Section::Effects && !self.effects.is_empty() {
                    let idx = local_idx.min(self.effects.len() - 1);
                    self.effects.remove(idx);
                    return self.emit_update();
                }
                Action::None
            }
            "toggle_poly" => {
                self.polyphonic = !self.polyphonic;
                self.emit_update()
            }
            "toggle_active" => {
                if self.source.is_audio_input() {
                    self.active = !self.active;
                    self.emit_update()
                } else {
                    Action::None
                }
            }
            "load_sample" => {
                if self.source.is_sample() {
                    if let Some(id) = self.instrument_id {
                        Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::LoadPitchedSample(id)))
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }
            "zero_param" => {
                self.zero_current_param();
                self.emit_update()
            }
            "zero_section" => {
                self.zero_current_section();
                self.emit_update()
            }
            "toggle_lfo" => {
                self.lfo.enabled = !self.lfo.enabled;
                self.emit_update()
            }
            "cycle_lfo_shape" => {
                self.lfo.shape = self.lfo.shape.next();
                self.emit_update()
            }
            "cycle_lfo_target" => {
                self.lfo.target = self.lfo.target.next();
                self.emit_update()
            }
            "next_section" => {
                // Jump to first row of next section
                let current = self.current_section();
                let skip_env = self.source.is_vst();
                let next = match current {
                    Section::Source => Section::Filter,
                    Section::Filter => Section::Effects,
                    Section::Effects => Section::Lfo,
                    Section::Lfo => if skip_env { Section::Source } else { Section::Envelope },
                    Section::Envelope => Section::Source,
                };
                for i in 0..self.total_rows() {
                    if self.section_for_row(i) == next {
                        self.selected_row = i;
                        break;
                    }
                }
                Action::None
            }
            "prev_section" => {
                // Jump to first row of previous section
                let current = self.current_section();
                let skip_env = self.source.is_vst();
                let prev = match current {
                    Section::Source => if skip_env { Section::Lfo } else { Section::Envelope },
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
                Action::None
            }
            _ => Action::None,
        }
    }

    pub(super) fn handle_raw_input_impl(&mut self, event: &InputEvent) {
        if self.editing {
            self.edit_input.handle_input(event);
        }
    }

    pub(super) fn handle_mouse_impl(&mut self, event: &crate::ui::MouseEvent) -> Action {
        let total = self.total_rows();
        if total == 0 { return Action::None; }

        match event.kind {
            crate::ui::MouseEventKind::ScrollUp => {
                self.selected_row = if self.selected_row == 0 { total - 1 } else { self.selected_row - 1 };
                Action::None
            }
            crate::ui::MouseEventKind::ScrollDown => {
                self.selected_row = (self.selected_row + 1) % total;
                Action::None
            }
            _ => Action::None,
        }
    }
}
