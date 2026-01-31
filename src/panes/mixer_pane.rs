use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, MixerSelection, OutputTarget};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, MouseEvent, MouseEventKind, MouseButton, MixerAction, Pane, Style};

const CHANNEL_WIDTH: u16 = 8;
const METER_HEIGHT: u16 = 12;
const NUM_VISIBLE_CHANNELS: usize = 8;
const NUM_VISIBLE_BUSES: usize = 2;

/// Block characters for vertical meter
const BLOCK_CHARS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

pub struct MixerPane {
    keymap: Keymap,
    send_target: Option<u8>,
}

impl MixerPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            send_target: None,
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

impl Pane for MixerPane {
    fn id(&self) -> &'static str {
        "mixer"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
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
            "clear_send" => { self.send_target = None; Action::None }
            _ => Action::None,
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action {
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

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_mixer_buf(buf, area, state);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl MixerPane {
    fn calc_scroll_offset(selected: usize, total: usize, visible: usize) -> usize {
        if selected >= visible {
            (selected - visible + 1).min(total.saturating_sub(visible))
        } else {
            0
        }
    }

    fn render_mixer_buf(&self, buf: &mut Buffer, area: RatatuiRect, state: &AppState) {
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

                Self::render_channel_buf(
                    buf, x, &format!("I{}", instrument.id), &instrument.name,
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
                        ))).render(RatatuiRect::new(base_x, send_y, rect.width.saturating_sub(4), 1), buf);
                    }
                }
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        Paragraph::new(Line::from(Span::styled(
            "[\u{2190}/\u{2192}] Select  [\u{2191}/\u{2193}] Level  [M]ute [S]olo [o]ut  [t/T] Send  [g] Toggle",
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(RatatuiRect::new(base_x, help_y, rect.width.saturating_sub(4), 1), buf);
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
