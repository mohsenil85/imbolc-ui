use super::{MixerPane, MixerSection};
use super::{CHANNEL_WIDTH, NUM_VISIBLE_CHANNELS, NUM_VISIBLE_BUSES, METER_HEIGHT};
use crate::state::{AppState, InstrumentId, MixerSelection};
use crate::ui::{Rect, Action, InputEvent, MixerAction, InstrumentAction, NavAction, MouseEvent, MouseEventKind, MouseButton};
use crate::ui::layout_helpers::center_rect;

impl MixerPane {
    pub(super) fn handle_action_impl(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        // Detail mode handling
        if self.detail_mode.is_some() {
            return self.handle_detail_action(action, state);
        }

        // Overview mode handling
        match action {
            "prev" => { self.send_target = None; Action::Mixer(MixerAction::Move(-1)) }
            "next" => { self.send_target = None; Action::Mixer(MixerAction::Move(1)) }
            "first" => Action::Mixer(MixerAction::Jump(1)),
            "last" => Action::Mixer(MixerAction::Jump(-1)),
            "level_up" => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, 0.05))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(0.05))
                }
            }
            "level_down" => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, -0.05))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(-0.05))
                }
            }
            "level_up_big" => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, 0.10))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(0.10))
                }
            }
            "level_down_big" => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, -0.10))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(-0.10))
                }
            }
            "mute" => Action::Mixer(MixerAction::ToggleMute),
            "solo" => Action::Mixer(MixerAction::ToggleSolo),
            "output" => Action::Mixer(MixerAction::CycleOutput),
            "output_rev" => Action::Mixer(MixerAction::CycleOutputReverse),
            "section" => { self.send_target = None; Action::Mixer(MixerAction::CycleSection) }
            "send_next" => {
                self.send_target = match self.send_target {
                    None => Some(1),
                    Some(8) => None,
                    Some(n) => Some(n + 1),
                };
                Action::None
            }
            "send_prev" => {
                self.send_target = match self.send_target {
                    None => Some(8),
                    Some(1) => None,
                    Some(n) => Some(n - 1),
                };
                Action::None
            }
            "send_toggle" => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::ToggleSend(bus_id))
                } else {
                    Action::None
                }
            }
            "clear_send" | "escape" => { self.send_target = None; Action::None }
            "enter_detail" => {
                if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                    if idx < state.instruments.instruments.len() {
                        self.detail_mode = Some(idx);
                        self.detail_section = MixerSection::Effects;
                        self.detail_cursor = 0;
                        self.effect_scroll = 0;
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    pub(super) fn handle_mouse_impl(&mut self, event: &MouseEvent, area: Rect, state: &AppState) -> Action {
        let box_width = (NUM_VISIBLE_CHANNELS as u16 * CHANNEL_WIDTH) + 2 +
                        (NUM_VISIBLE_BUSES as u16 * CHANNEL_WIDTH) + 2 +
                        CHANNEL_WIDTH + 4;
        let box_height = METER_HEIGHT + 8;
        let rect = center_rect(area, box_width, box_height);
        let base_x = rect.x + 2;

        let col = event.column;
        let row = event.row;

        // Check if click is within the mixer box
        if col < rect.x || col >= rect.x + rect.width || row < rect.y || row >= rect.y + rect.height {
            return Action::None;
        }

        // Calculate scroll offsets (same as render)
        let instrument_scroll = match state.session.mixer_selection {
            MixerSelection::Instrument(idx) => {
                Self::calc_scroll_offset(idx, state.instruments.instruments.len(), NUM_VISIBLE_CHANNELS)
            }
            _ => 0,
        };
        let bus_scroll = match state.session.mixer_selection {
            MixerSelection::Bus(id) => {
                Self::calc_scroll_offset((id - 1) as usize, state.session.buses.len(), NUM_VISIBLE_BUSES)
            }
            _ => 0,
        };

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Instrument channels region
                let inst_end_x = base_x + (NUM_VISIBLE_CHANNELS as u16 * CHANNEL_WIDTH);
                if col >= base_x && col < inst_end_x {
                    let channel = ((col - base_x) / CHANNEL_WIDTH) as usize;
                    let idx = instrument_scroll + channel;
                    if idx < state.instruments.instruments.len() {
                        self.send_target = None;
                        return Action::Mixer(MixerAction::SelectAt(MixerSelection::Instrument(idx)));
                    }
                }

                // Bus channels region (after separator)
                let bus_start_x = inst_end_x + 2;
                let bus_end_x = bus_start_x + (NUM_VISIBLE_BUSES as u16 * CHANNEL_WIDTH);
                if col >= bus_start_x && col < bus_end_x {
                    let channel = ((col - bus_start_x) / CHANNEL_WIDTH) as usize;
                    let bus_idx = bus_scroll + channel;
                    if bus_idx < state.session.buses.len() {
                        let bus_id = state.session.buses[bus_idx].id;
                        self.send_target = None;
                        return Action::Mixer(MixerAction::SelectAt(MixerSelection::Bus(bus_id)));
                    }
                }

                // Master region (after second separator)
                let master_start_x = bus_end_x + 2;
                if col >= master_start_x {
                    self.send_target = None;
                    return Action::Mixer(MixerAction::SelectAt(MixerSelection::Master));
                }

                Action::None
            }
            MouseEventKind::ScrollUp => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, 0.05))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(0.05))
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(bus_id) = self.send_target {
                    Action::Mixer(MixerAction::AdjustSend(bus_id, -0.05))
                } else {
                    Action::Mixer(MixerAction::AdjustLevel(-0.05))
                }
            }
            _ => Action::None,
        }
    }

    fn handle_detail_action(&mut self, action: &str, state: &AppState) -> Action {
        let Some(inst_id) = self.detail_instrument_id(state) else {
            self.detail_mode = None;
            return Action::None;
        };

        match action {
            "escape" | "clear_send" => {
                self.detail_mode = None;
                self.send_target = None;
                Action::None
            }
            "section" => {
                self.detail_section = self.detail_section.next();
                self.detail_cursor = 0;
                Action::None
            }
            "section_prev" => {
                self.detail_section = self.detail_section.prev();
                self.detail_cursor = 0;
                Action::None
            }
            "level_up" | "prev" => {
                if self.detail_cursor > 0 {
                    self.detail_cursor -= 1;
                }
                Action::None
            }
            "level_down" | "next" => {
                let max = self.max_cursor(state);
                if self.detail_cursor < max {
                    self.detail_cursor += 1;
                }
                Action::None
            }
            "level_up_big" | "first" => {
                self.adjust_detail_param(state, inst_id, 5.0)
            }
            "level_down_big" | "last" => {
                self.adjust_detail_param(state, inst_id, -5.0)
            }
            "increase" | "fine_right" => {
                self.adjust_detail_param(state, inst_id, 1.0)
            }
            "decrease" | "fine_left" => {
                self.adjust_detail_param(state, inst_id, -1.0)
            }
            "mute" => Action::Mixer(MixerAction::ToggleMute),
            "solo" => Action::Mixer(MixerAction::ToggleSolo),
            "output" => Action::Mixer(MixerAction::CycleOutput),
            "output_rev" => Action::Mixer(MixerAction::CycleOutputReverse),
            "add_effect" => {
                Action::Nav(NavAction::PushPane("add_effect"))
            }
            "remove_effect" => {
                if self.detail_section == MixerSection::Effects {
                    if let Some((ei, _)) = self.decode_effect_cursor(state) {
                        let max_after = self.max_cursor(state).saturating_sub(1);
                        if self.detail_cursor > max_after {
                            self.detail_cursor = max_after;
                        }
                        return Action::Instrument(InstrumentAction::RemoveEffect(inst_id, ei));
                    }
                }
                Action::None
            }
            "toggle_effect" => {
                if self.detail_section == MixerSection::Effects {
                    if let Some((ei, _)) = self.decode_effect_cursor(state) {
                        return Action::Instrument(InstrumentAction::ToggleEffectBypass(inst_id, ei));
                    }
                }
                Action::None
            }
            "toggle_filter" => {
                Action::Instrument(InstrumentAction::ToggleFilter(inst_id))
            }
            "cycle_filter_type" => {
                Action::Instrument(InstrumentAction::CycleFilterType(inst_id))
            }
            "move_up" => {
                if self.detail_section == MixerSection::Effects {
                    if let Some((ei, _)) = self.decode_effect_cursor(state) {
                        if ei > 0 {
                            return Action::Instrument(InstrumentAction::MoveEffect(inst_id, ei, -1));
                        }
                    }
                }
                Action::None
            }
            "move_down" => {
                if self.detail_section == MixerSection::Effects {
                    if let Some((ei, _)) = self.decode_effect_cursor(state) {
                        return Action::Instrument(InstrumentAction::MoveEffect(inst_id, ei, 1));
                    }
                }
                Action::None
            }
            "pan_left" => Action::Mixer(MixerAction::AdjustPan(-0.05)),
            "pan_right" => Action::Mixer(MixerAction::AdjustPan(0.05)),
            "enter_detail" => {
                match self.detail_section {
                    MixerSection::Effects => {
                        Action::None
                    }
                    _ => Action::None,
                }
            }
            "send_next" => {
                if self.detail_section == MixerSection::Sends {
                    let max = self.max_cursor(state);
                    if self.detail_cursor < max {
                        self.detail_cursor += 1;
                    }
                }
                Action::None
            }
            "send_prev" => {
                if self.detail_section == MixerSection::Sends {
                    if self.detail_cursor > 0 {
                        self.detail_cursor -= 1;
                    }
                }
                Action::None
            }
            "send_toggle" => {
                if self.detail_section == MixerSection::Sends {
                    if let Some((_, inst)) = self.detail_instrument(state) {
                        if let Some(send) = inst.sends.get(self.detail_cursor) {
                            return Action::Mixer(MixerAction::ToggleSend(send.bus_id));
                        }
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn adjust_detail_param(&self, state: &AppState, inst_id: InstrumentId, delta: f32) -> Action {
        match self.detail_section {
            MixerSection::Effects => {
                if let Some((ei, Some(pi))) = self.decode_effect_cursor(state) {
                    return Action::Instrument(InstrumentAction::AdjustEffectParam(inst_id, ei, pi, delta));
                }
                Action::None
            }
            MixerSection::Sends => {
                if let Some((_, inst)) = self.detail_instrument(state) {
                    if let Some(send) = inst.sends.get(self.detail_cursor) {
                        return Action::Mixer(MixerAction::AdjustSend(send.bus_id, delta * 0.01));
                    }
                }
                Action::None
            }
            MixerSection::Filter => {
                match self.detail_cursor {
                    0 => Action::Instrument(InstrumentAction::CycleFilterType(inst_id)),
                    1 => Action::Instrument(InstrumentAction::AdjustFilterCutoff(inst_id, delta)),
                    2 => Action::Instrument(InstrumentAction::AdjustFilterResonance(inst_id, delta)),
                    _ => Action::None,
                }
            }
            MixerSection::Lfo => {
                Action::None
            }
            MixerSection::Output => {
                match self.detail_cursor {
                    0 => Action::Mixer(MixerAction::AdjustPan(delta * 0.01)),
                    1 => Action::Mixer(MixerAction::AdjustLevel(delta * 0.01)),
                    2 => {
                        if delta > 0.0 {
                            Action::Mixer(MixerAction::CycleOutput)
                        } else {
                            Action::Mixer(MixerAction::CycleOutputReverse)
                        }
                    }
                    _ => Action::None,
                }
            }
        }
    }
}
