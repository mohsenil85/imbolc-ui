use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, InstrumentId, MixerSelection, OutputTarget};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Rect, RenderBuf, Action, Color, InputEvent, InstrumentAction, Keymap, MouseEvent, MouseEventKind, MouseButton, MixerAction, NavAction, Pane, Style};

const CHANNEL_WIDTH: u16 = 8;
const METER_HEIGHT: u16 = 12;
const NUM_VISIBLE_CHANNELS: usize = 8;
const NUM_VISIBLE_BUSES: usize = 2;

/// Block characters for vertical meter
const BLOCK_CHARS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixerSection {
    Effects,
    Sends,
    Filter,
    Lfo,
    Output,
}

impl MixerSection {
    fn next(self) -> Self {
        match self {
            Self::Effects => Self::Sends,
            Self::Sends => Self::Filter,
            Self::Filter => Self::Lfo,
            Self::Lfo => Self::Output,
            Self::Output => Self::Effects,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Effects => Self::Output,
            Self::Sends => Self::Effects,
            Self::Filter => Self::Sends,
            Self::Lfo => Self::Filter,
            Self::Output => Self::Lfo,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Effects => "EFFECTS",
            Self::Sends => "SENDS",
            Self::Filter => "FILTER",
            Self::Lfo => "LFO",
            Self::Output => "OUTPUT",
        }
    }
}

pub struct MixerPane {
    keymap: Keymap,
    send_target: Option<u8>,
    detail_mode: Option<usize>,
    detail_section: MixerSection,
    detail_cursor: usize,
    effect_scroll: usize,
}

impl MixerPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            send_target: None,
            detail_mode: None,
            detail_section: MixerSection::Effects,
            detail_cursor: 0,
            effect_scroll: 0,
        }
    }

    fn level_to_db(level: f32) -> String {
        if level <= 0.0 {
            "-\u{221e}".to_string()
        } else {
            let db = 20.0 * level.log10();
            format!("{:+.0}", db.max(-99.0))
        }
    }

    fn meter_color(row: u16, height: u16) -> Color {
        let frac = row as f32 / height as f32;
        if frac > 0.85 {
            Color::METER_HIGH
        } else if frac > 0.6 {
            Color::METER_MID
        } else {
            Color::METER_LOW
        }
    }

    fn format_output(target: OutputTarget) -> &'static str {
        match target {
            OutputTarget::Master => ">MST",
            OutputTarget::Bus(1) => ">B1",
            OutputTarget::Bus(2) => ">B2",
            OutputTarget::Bus(3) => ">B3",
            OutputTarget::Bus(4) => ">B4",
            OutputTarget::Bus(5) => ">B5",
            OutputTarget::Bus(6) => ">B6",
            OutputTarget::Bus(7) => ">B7",
            OutputTarget::Bus(8) => ">B8",
            OutputTarget::Bus(_) => ">??",
        }
    }

    #[allow(dead_code)]
    pub fn send_target(&self) -> Option<u8> {
        self.send_target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::ui::{InputEvent, KeyCode, Modifiers};

    fn dummy_event() -> InputEvent {
        InputEvent::new(KeyCode::Char('x'), Modifiers::default())
    }

    #[test]
    fn send_target_cycles_and_adjusts_send() {
        let mut pane = MixerPane::new(Keymap::new());
        let state = AppState::new();

        let action = pane.handle_action("send_next", &dummy_event(), &state);
        assert!(matches!(action, Action::None));
        assert_eq!(pane.send_target, Some(1));

        let action = pane.handle_action("level_up", &dummy_event(), &state);
        match action {
            Action::Mixer(MixerAction::AdjustSend(bus_id, delta)) => {
                assert_eq!(bus_id, 1);
                assert!((delta - 0.05).abs() < 0.0001);
            }
            _ => panic!("Expected AdjustSend when send_target is set"),
        }

        let action = pane.handle_action("clear_send", &dummy_event(), &state);
        assert!(matches!(action, Action::None));
        assert_eq!(pane.send_target, None);
    }

    #[test]
    fn prev_next_clear_send_target() {
        let mut pane = MixerPane::new(Keymap::new());
        let state = AppState::new();

        pane.send_target = Some(3);
        let action = pane.handle_action("prev", &dummy_event(), &state);
        assert!(matches!(action, Action::Mixer(MixerAction::Move(-1))));
        assert_eq!(pane.send_target, None);

        pane.send_target = Some(2);
        let action = pane.handle_action("next", &dummy_event(), &state);
        assert!(matches!(action, Action::Mixer(MixerAction::Move(1))));
        assert_eq!(pane.send_target, None);
    }
}

impl Default for MixerPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl MixerPane {
    /// Get the instrument index and ID for the current detail mode target
    fn detail_instrument<'a>(&self, state: &'a AppState) -> Option<(usize, &'a crate::state::Instrument)> {
        let idx = self.detail_mode?;
        state.instruments.instruments.get(idx).map(|inst| (idx, inst))
    }

    fn detail_instrument_id(&self, state: &AppState) -> Option<InstrumentId> {
        self.detail_instrument(state).map(|(_, inst)| inst.id)
    }

    /// Max cursor position for current section
    fn max_cursor(&self, state: &AppState) -> usize {
        let Some((_, inst)) = self.detail_instrument(state) else { return 0 };
        match self.detail_section {
            MixerSection::Effects => {
                if inst.effects.is_empty() { 0 }
                else {
                    // cursor indexes: effect_idx * (1 + param_count) for effect header, then params
                    let mut count = 0;
                    for effect in &inst.effects {
                        count += 1 + effect.params.len(); // header + params
                    }
                    count.saturating_sub(1)
                }
            }
            MixerSection::Sends => inst.sends.len().saturating_sub(1),
            MixerSection::Filter => {
                if inst.filter.is_some() { 2 } else { 0 } // type, cutoff, resonance
            }
            MixerSection::Lfo => 2, // rate, depth, shape
            MixerSection::Output => 2, // pan, level, output target
        }
    }

    /// Decode effect cursor into (effect_index, param_index_within_effect) where None = header
    fn decode_effect_cursor(&self, state: &AppState) -> Option<(usize, Option<usize>)> {
        let (_, inst) = self.detail_instrument(state)?;
        let mut pos = 0;
        for (ei, effect) in inst.effects.iter().enumerate() {
            if self.detail_cursor == pos {
                return Some((ei, None)); // on effect header
            }
            pos += 1;
            for pi in 0..effect.params.len() {
                if self.detail_cursor == pos {
                    return Some((ei, Some(pi)));
                }
                pos += 1;
            }
        }
        None
    }
}

impl Pane for MixerPane {
    fn id(&self) -> &'static str {
        "mixer"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
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

    fn handle_mouse(&mut self, event: &MouseEvent, area: Rect, state: &AppState) -> Action {
        use crate::state::MixerSelection;

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

    fn render(&mut self, area: Rect, buf: &mut RenderBuf, state: &AppState) {
        let buf = buf.raw_buf();
        if self.detail_mode.is_some() {
            self.render_detail_buf(buf, area, state);
        } else {
            self.render_mixer_buf(buf, area, state);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl MixerPane {
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
                // Navigate up within section
                if self.detail_cursor > 0 {
                    self.detail_cursor -= 1;
                }
                Action::None
            }
            "level_down" | "next" => {
                // Navigate down within section
                let max = self.max_cursor(state);
                if self.detail_cursor < max {
                    self.detail_cursor += 1;
                }
                Action::None
            }
            "level_up_big" | "first" => {
                // Adjust current param up (coarse)
                self.adjust_detail_param(state, inst_id, 5.0)
            }
            "level_down_big" | "last" => {
                // Adjust current param down (coarse)
                self.adjust_detail_param(state, inst_id, -5.0)
            }
            "increase" | "fine_right" => {
                // Adjust current param right (fine)
                self.adjust_detail_param(state, inst_id, 1.0)
            }
            "decrease" | "fine_left" => {
                // Adjust current param left (fine)
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
                        // Adjust cursor if removing last effect
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
                // In detail mode, Enter on EQ section opens EQ pane, etc.
                match self.detail_section {
                    MixerSection::Effects => {
                        // Could open VST params if it's a VST effect
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
                // LFO adjustments could be added later
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

    fn calc_scroll_offset(selected: usize, total: usize, visible: usize) -> usize {
        if selected >= visible {
            (selected - visible + 1).min(total.saturating_sub(visible))
        } else {
            0
        }
    }

    fn render_mixer_buf(&self, buf: &mut Buffer, area: Rect, state: &AppState) {
        let box_width = (NUM_VISIBLE_CHANNELS as u16 * CHANNEL_WIDTH) + 2 +
                        (NUM_VISIBLE_BUSES as u16 * CHANNEL_WIDTH) + 2 +
                        CHANNEL_WIDTH + 4;
        let box_height = METER_HEIGHT + 8;
        let rect = center_rect(area, box_width, box_height);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" MIXER ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        block.render(rect, buf);

        let base_x = rect.x + 2;
        let base_y = rect.y + 1;

        let label_y = base_y;
        let name_y = base_y + 1;
        let meter_top_y = base_y + 2;
        let db_y = meter_top_y + METER_HEIGHT;
        let indicator_y = db_y + 1;
        let output_y = indicator_y + 1;

        // Calculate scroll offsets
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

        let mut x = base_x;

        // Render instrument channels
        for i in 0..NUM_VISIBLE_CHANNELS {
            let idx = instrument_scroll + i;
            if idx < state.instruments.instruments.len() {
                let instrument = &state.instruments.instruments[idx];
                let is_selected = matches!(state.session.mixer_selection, MixerSelection::Instrument(s) if s == idx);

                let label = if instrument.layer_group.is_some() {
                    format!("I{}L", instrument.id)
                } else {
                    format!("I{}", instrument.id)
                };
                Self::render_channel_buf(
                    buf, x, &label, &instrument.name,
                    instrument.level, instrument.mute, instrument.solo, Some(instrument.output_target), is_selected,
                    label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
                );
            } else {
                Self::render_empty_channel_buf(
                    buf, x, &format!("I{}", idx + 1),
                    label_y, name_y, meter_top_y, db_y, indicator_y,
                );
            }

            x += CHANNEL_WIDTH;
        }

        // Separator before buses
        let purple_style = ratatui::style::Style::from(Style::new().fg(Color::PURPLE));
        for y in label_y..=output_y {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('│').set_style(purple_style);
            }
        }
        x += 2;

        // Render buses
        for i in 0..NUM_VISIBLE_BUSES {
            let bus_idx = bus_scroll + i;
            if bus_idx >= state.session.buses.len() {
                break;
            }
            let bus = &state.session.buses[bus_idx];
            let is_selected = matches!(state.session.mixer_selection, MixerSelection::Bus(id) if id == bus.id);

            Self::render_channel_buf(
                buf, x, &format!("BUS{}", bus.id), &bus.name,
                bus.level, bus.mute, bus.solo, None, is_selected,
                label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
            );

            x += CHANNEL_WIDTH;
        }

        // Separator before master
        let gold_style = ratatui::style::Style::from(Style::new().fg(Color::GOLD));
        for y in label_y..=output_y {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('│').set_style(gold_style);
            }
        }
        x += 2;

        // Master
        let is_master_selected = matches!(state.session.mixer_selection, MixerSelection::Master);
        Self::render_channel_buf(
            buf, x, "MASTER", "",
            state.session.master_level, state.session.master_mute, false, None, is_master_selected,
            label_y, name_y, meter_top_y, db_y, indicator_y, output_y,
        );

        // Send info line
        let send_y = output_y + 1;
        if let Some(bus_id) = self.send_target {
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                if let Some(instrument) = state.instruments.instruments.get(idx) {
                    if let Some(send) = instrument.sends.iter().find(|s| s.bus_id == bus_id) {
                        let status = if send.enabled { "ON" } else { "OFF" };
                        let info = format!("Send→B{}: {:.0}% [{}]", bus_id, send.level * 100.0, status);
                        Paragraph::new(Line::from(Span::styled(
                            info,
                            ratatui::style::Style::from(Style::new().fg(Color::TEAL).bold()),
                        ))).render(Rect::new(base_x, send_y, rect.width.saturating_sub(4), 1), buf);
                    }
                }
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        Paragraph::new(Line::from(Span::styled(
            "[\u{2190}/\u{2192}] Select  [\u{2191}/\u{2193}] Level  [M]ute [S]olo [o]ut  [t/T] Send  [g] Toggle",
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(Rect::new(base_x, help_y, rect.width.saturating_sub(4), 1), buf);
    }

    fn render_detail_buf(&self, buf: &mut Buffer, area: Rect, state: &AppState) {
        let Some((_, inst)) = self.detail_instrument(state) else {
            return;
        };

        let source_label = format!("{:?}", inst.source).chars().take(12).collect::<String>();
        let title = format!(" MIXER --- I{}: {} [{}] ", inst.id, inst.name, source_label);

        let box_width = area.width.min(90);
        let box_height = area.height.min(28);
        let rect = center_rect(area, box_width, box_height);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        block.render(rect, buf);

        let inner_x = rect.x + 2;
        let inner_y = rect.y + 1;
        let inner_w = rect.width.saturating_sub(4);
        let inner_h = rect.height.saturating_sub(3);

        // 3-column layout
        let col1_w = inner_w * 40 / 100; // Effects
        let col2_w = inner_w * 28 / 100; // Sends + Filter
        let _col3_w = inner_w.saturating_sub(col1_w + col2_w + 2); // Output + LFO

        let col1_x = inner_x;
        let col2_x = col1_x + col1_w + 1;
        let col3_x = col2_x + col2_w + 1;

        let dim = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let normal = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
        let header_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold());
        let active_section = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold());
        let selected_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));

        // Column separators
        for y in inner_y..(inner_y + inner_h) {
            if let Some(cell) = buf.cell_mut((col2_x - 1, y)) {
                cell.set_char('│').set_style(dim);
            }
            if let Some(cell) = buf.cell_mut((col3_x - 1, y)) {
                cell.set_char('│').set_style(dim);
            }
        }

        // ── Column 1: Effects Chain ──
        let effects_header = if self.detail_section == MixerSection::Effects {
            active_section
        } else {
            header_style
        };
        Self::write_str(buf, col1_x, inner_y, "EFFECTS CHAIN", effects_header);

        let mut ey = inner_y + 1;
        let mut cursor_pos = 0;
        for (ei, effect) in inst.effects.iter().enumerate() {
            if ey >= inner_y + inner_h { break; }

            let bypass_char = if effect.enabled { '\u{25CF}' } else { '\u{25CB}' }; // ● or ○
            let effect_label = format!("{} [{}] {:?}", ei + 1, bypass_char, effect.effect_type);
            let style = if self.detail_section == MixerSection::Effects && self.detail_cursor == cursor_pos {
                selected_style
            } else {
                normal
            };
            Self::write_str(buf, col1_x, ey, &effect_label, style);
            ey += 1;
            cursor_pos += 1;

            // Show up to 4 key params
            for (pi, param) in effect.params.iter().take(4).enumerate() {
                if ey >= inner_y + inner_h { break; }
                let val_str = match &param.value {
                    crate::state::ParamValue::Float(v) => format!("{:.2}", v),
                    crate::state::ParamValue::Int(v) => format!("{}", v),
                    crate::state::ParamValue::Bool(b) => if *b { "ON".to_string() } else { "OFF".to_string() },
                };
                let param_text = format!("  {} {}", param.name, val_str);
                let pstyle = if self.detail_section == MixerSection::Effects && self.detail_cursor == cursor_pos {
                    selected_style
                } else {
                    dim
                };
                Self::write_str(buf, col1_x + 1, ey, &param_text, pstyle);
                ey += 1;
                cursor_pos += 1;
                let _ = pi; // used in enumerate
            }
        }
        if inst.effects.is_empty() {
            Self::write_str(buf, col1_x, ey, "(no effects)", dim);
        }

        // ── Column 2 top: Sends ──
        let sends_header = if self.detail_section == MixerSection::Sends {
            active_section
        } else {
            header_style
        };
        Self::write_str(buf, col2_x, inner_y, "SENDS", sends_header);

        let mut sy = inner_y + 1;
        for (si, send) in inst.sends.iter().enumerate() {
            if sy >= inner_y + inner_h / 2 { break; }
            let bar_len = (send.level * 5.0) as usize;
            let bar: String = "\u{2588}".repeat(bar_len) + &"\u{2591}".repeat(5 - bar_len);
            let status = if send.enabled {
                format!("{:.0}%", send.level * 100.0)
            } else {
                "OFF".to_string()
            };
            let send_text = format!("\u{2192}B{} {} {}", send.bus_id, bar, status);
            let sstyle = if self.detail_section == MixerSection::Sends && self.detail_cursor == si {
                selected_style
            } else if send.enabled {
                normal
            } else {
                dim
            };
            Self::write_str(buf, col2_x, sy, &send_text, sstyle);
            sy += 1;
        }

        // ── Column 2 bottom: Filter ──
        let filter_y = inner_y + inner_h / 2;
        let filter_header = if self.detail_section == MixerSection::Filter {
            active_section
        } else {
            header_style
        };
        Self::write_str(buf, col2_x, filter_y, "FILTER", filter_header);

        let mut fy = filter_y + 1;
        if let Some(ref filter) = inst.filter {
            let type_text = format!("{:?}", filter.filter_type);
            let type_style = if self.detail_section == MixerSection::Filter && self.detail_cursor == 0 {
                selected_style
            } else {
                normal
            };
            Self::write_str(buf, col2_x, fy, &type_text, type_style);
            fy += 1;

            let cut_text = format!("Cut: {:.0} Hz", filter.cutoff.value);
            let cut_style = if self.detail_section == MixerSection::Filter && self.detail_cursor == 1 {
                selected_style
            } else {
                dim
            };
            Self::write_str(buf, col2_x, fy, &cut_text, cut_style);
            fy += 1;

            let res_text = format!("Res: {:.2}", filter.resonance.value);
            let res_style = if self.detail_section == MixerSection::Filter && self.detail_cursor == 2 {
                selected_style
            } else {
                dim
            };
            Self::write_str(buf, col2_x, fy, &res_text, res_style);
        } else {
            Self::write_str(buf, col2_x, fy, "(off)", dim);
        }

        // ── Column 3 top: Output ──
        let output_header = if self.detail_section == MixerSection::Output {
            active_section
        } else {
            header_style
        };
        Self::write_str(buf, col3_x, inner_y, "OUTPUT", output_header);

        let mut oy = inner_y + 1;

        // Pan
        let pan_text = format!("Pan: {:+.2}", inst.pan);
        let pan_style = if self.detail_section == MixerSection::Output && self.detail_cursor == 0 {
            selected_style
        } else {
            normal
        };
        Self::write_str(buf, col3_x, oy, &pan_text, pan_style);
        oy += 1;

        // Level with mini meter
        let db_str = Self::level_to_db(inst.level);
        let meter_len = (inst.level * 10.0) as usize;
        let meter_bar: String = "\u{258E}".repeat(meter_len) + &"\u{2591}".repeat(10usize.saturating_sub(meter_len));
        let level_text = format!("{} {}", meter_bar, db_str);
        let level_style = if self.detail_section == MixerSection::Output && self.detail_cursor == 1 {
            selected_style
        } else {
            normal
        };
        Self::write_str(buf, col3_x, oy, &level_text, level_style);
        oy += 1;

        // Output target
        let out_text = format!("\u{25B8} {}", match inst.output_target {
            OutputTarget::Master => "Master".to_string(),
            OutputTarget::Bus(id) => format!("Bus {}", id),
        });
        let out_style = if self.detail_section == MixerSection::Output && self.detail_cursor == 2 {
            selected_style
        } else {
            dim
        };
        Self::write_str(buf, col3_x, oy, &out_text, out_style);
        oy += 1;

        // Mute/Solo indicators
        let mute_str = if inst.mute { "[M]" } else { " M " };
        let solo_str = if inst.solo { "[S]" } else { " S " };
        let mute_style = if inst.mute {
            ratatui::style::Style::from(Style::new().fg(Color::MUTE_COLOR).bold())
        } else {
            dim
        };
        let solo_style = if inst.solo {
            ratatui::style::Style::from(Style::new().fg(Color::SOLO_COLOR).bold())
        } else {
            dim
        };
        Self::write_str(buf, col3_x, oy, mute_str, mute_style);
        Self::write_str(buf, col3_x + 4, oy, solo_str, solo_style);

        // ── Column 3 bottom: LFO ──
        let lfo_y = inner_y + inner_h / 2;
        let lfo_header = if self.detail_section == MixerSection::Lfo {
            active_section
        } else {
            header_style
        };
        Self::write_str(buf, col3_x, lfo_y, "LFO", lfo_header);

        let mut ly = lfo_y + 1;
        let lfo = &inst.lfo;
        if lfo.enabled {
            let shape_text = format!("{:?} {:.1}Hz", lfo.shape, lfo.rate);
            let shape_style = if self.detail_section == MixerSection::Lfo && self.detail_cursor == 0 {
                selected_style
            } else {
                normal
            };
            Self::write_str(buf, col3_x, ly, &shape_text, shape_style);
            ly += 1;

            let depth_text = format!("Depth: {:.2}", lfo.depth);
            let depth_style = if self.detail_section == MixerSection::Lfo && self.detail_cursor == 1 {
                selected_style
            } else {
                dim
            };
            Self::write_str(buf, col3_x, ly, &depth_text, depth_style);
            ly += 1;

            let target_text = format!("Tgt: {:?}", lfo.target);
            let target_style = if self.detail_section == MixerSection::Lfo && self.detail_cursor == 2 {
                selected_style
            } else {
                dim
            };
            Self::write_str(buf, col3_x, ly, &target_text, target_style);
        } else {
            Self::write_str(buf, col3_x, ly, "(off)", dim);
        }

        // ── Help bar ──
        let help_y = rect.y + rect.height - 2;
        let help_text = "Tab: Section  \u{2191}/\u{2193}: Nav  PageUp/Dn: Adjust  [a]dd [d]el [e] Bypass  [f]ilter  [p/P] Pan  Esc: Back";
        Paragraph::new(Line::from(Span::styled(
            help_text,
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(Rect::new(inner_x, help_y, inner_w, 1), buf);

        // Section indicator bar (just below title)
        let section_bar_y = rect.y;
        let sections = [MixerSection::Effects, MixerSection::Sends, MixerSection::Filter, MixerSection::Lfo, MixerSection::Output];
        let mut sx = rect.x + (title.len() as u16) + 1;
        for &section in &sections {
            if sx + section.label().len() as u16 + 2 >= rect.x + rect.width { break; }
            let sstyle = if section == self.detail_section {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            let label = format!(" {} ", section.label());
            Self::write_str(buf, sx, section_bar_y, &label, sstyle);
            sx += label.len() as u16 + 1;
        }
    }

    fn write_str(buf: &mut Buffer, x: u16, y: u16, text: &str, style: ratatui::style::Style) {
        for (i, ch) in text.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
                cell.set_char(ch).set_style(style);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_channel_buf(
        buf: &mut Buffer,
        x: u16,
        label: &str,
        name: &str,
        level: f32,
        mute: bool,
        solo: bool,
        output: Option<OutputTarget>,
        selected: bool,
        label_y: u16,
        name_y: u16,
        meter_top_y: u16,
        db_y: u16,
        indicator_y: u16,
        output_y: u16,
    ) {
        let channel_w = (CHANNEL_WIDTH - 1) as usize;

        let label_style = if selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
        } else if label.starts_with("BUS") {
            ratatui::style::Style::from(Style::new().fg(Color::PURPLE).bold())
        } else if label == "MASTER" {
            ratatui::style::Style::from(Style::new().fg(Color::GOLD).bold())
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::CYAN))
        };
        for (j, ch) in label.chars().take(channel_w).enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, label_y)) {
                cell.set_char(ch).set_style(label_style);
            }
        }

        let text_style = if selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
        };
        let name_display = if name.is_empty() && label.starts_with('I') { "---" } else { name };
        for (j, ch) in name_display.chars().take(channel_w).enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, name_y)) {
                cell.set_char(ch).set_style(text_style);
            }
        }

        // Vertical meter
        let meter_x = x + (CHANNEL_WIDTH / 2).saturating_sub(1);
        Self::render_meter_buf(buf, meter_x, meter_top_y, METER_HEIGHT, level);

        // Selection indicator
        if selected {
            let sel_x = meter_x + 1;
            if let Some(cell) = buf.cell_mut((sel_x, meter_top_y)) {
                cell.set_char('▼').set_style(
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold()),
                );
            }
        }

        // dB display
        let db_style = if selected {
            ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
        } else {
            ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE))
        };
        let db_str = Self::level_to_db(level);
        for (j, ch) in db_str.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, db_y)) {
                cell.set_char(ch).set_style(db_style);
            }
        }

        // Mute/Solo indicator
        let (indicator, indicator_style) = if mute {
            ("M", ratatui::style::Style::from(Style::new().fg(Color::MUTE_COLOR).bold()))
        } else if solo {
            ("S", ratatui::style::Style::from(Style::new().fg(Color::SOLO_COLOR).bold()))
        } else {
            ("●", ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)))
        };
        for (j, ch) in indicator.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, indicator_y)) {
                cell.set_char(ch).set_style(indicator_style);
            }
        }

        // Output routing
        if let Some(target) = output {
            let routing_style = if selected {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::TEAL))
            };
            for (j, ch) in Self::format_output(target).chars().enumerate() {
                if let Some(cell) = buf.cell_mut((x + j as u16, output_y)) {
                    cell.set_char(ch).set_style(routing_style);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_empty_channel_buf(
        buf: &mut Buffer,
        x: u16,
        label: &str,
        label_y: u16,
        name_y: u16,
        meter_top_y: u16,
        db_y: u16,
        indicator_y: u16,
    ) {
        let channel_w = (CHANNEL_WIDTH - 1) as usize;
        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));

        for (j, ch) in label.chars().take(channel_w).enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, label_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }
        for (j, ch) in "---".chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, name_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }

        let meter_x = x + (CHANNEL_WIDTH / 2).saturating_sub(1);
        for row in 0..METER_HEIGHT {
            if let Some(cell) = buf.cell_mut((meter_x, meter_top_y + row)) {
                cell.set_char('·').set_style(dark_gray);
            }
        }

        for (j, ch) in "--".chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, db_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }
        for (j, ch) in "●".chars().enumerate() {
            if let Some(cell) = buf.cell_mut((x + j as u16, indicator_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }
    }

    fn render_meter_buf(buf: &mut Buffer, x: u16, top_y: u16, height: u16, level: f32) {
        let total_sub = height as f32 * 8.0;
        let filled_sub = (level * total_sub) as u16;

        for row in 0..height {
            let inverted_row = height - 1 - row;
            let y = top_y + row;
            let row_start = inverted_row * 8;
            let row_end = row_start + 8;
            let color = Self::meter_color(inverted_row, height);

            let (ch, c) = if filled_sub >= row_end {
                ('\u{2588}', color)
            } else if filled_sub > row_start {
                let sub_level = (filled_sub - row_start) as usize;
                (BLOCK_CHARS[sub_level.saturating_sub(1).min(7)], color)
            } else {
                ('·', Color::DARK_GRAY)
            };

            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch).set_style(ratatui::style::Style::from(Style::new().fg(c)));
            }
        }
    }
}
