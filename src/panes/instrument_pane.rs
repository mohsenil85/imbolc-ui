use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, SourceType};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, NavAction, InstrumentAction, SessionAction, Color, InputEvent, KeyCode, Keymap, MouseEvent, MouseEventKind, MouseButton, PadKeyboard, Pane, PianoKeyboard, Style, ToggleResult, translate_key};

fn source_color(source: SourceType) -> Color {
    match source {
        SourceType::Saw | SourceType::Sin | SourceType::Sqr | SourceType::Tri
        | SourceType::Noise | SourceType::Pulse | SourceType::SuperSaw | SourceType::Sync
        | SourceType::Ring | SourceType::FBSin | SourceType::FM | SourceType::PhaseMod
        | SourceType::Pluck | SourceType::Formant | SourceType::Gendy | SourceType::Chaos
        | SourceType::Additive | SourceType::Wavetable => Color::OSC_COLOR,
        SourceType::AudioIn => Color::AUDIO_IN_COLOR,
        SourceType::PitchedSampler => Color::SAMPLE_COLOR,
        SourceType::Kit => Color::KIT_COLOR,
        SourceType::BusIn => Color::BUS_IN_COLOR,
        SourceType::Custom(_) => Color::CUSTOM_COLOR,
        SourceType::Vst(_) => Color::VST_COLOR,
    }
}

pub struct InstrumentPane {
    keymap: Keymap,
    piano: PianoKeyboard,
    pad_keyboard: PadKeyboard,
}

impl InstrumentPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            piano: PianoKeyboard::new(),
            pad_keyboard: PadKeyboard::new(),
        }
    }

    fn format_filter(instrument: &crate::state::instrument::Instrument) -> String {
        match &instrument.filter {
            Some(f) => format!("[{}]", f.filter_type.name()),
            None => "---".to_string(),
        }
    }

    fn format_eq(instrument: &crate::state::instrument::Instrument) -> &'static str {
        if instrument.eq.is_some() { "[EQ]" } else { "" }
    }

    fn format_effects(instrument: &crate::state::instrument::Instrument) -> String {
        if instrument.effects.is_empty() {
            return "---".to_string();
        }
        instrument.effects.iter()
            .map(|e| e.effect_type.name())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn format_level(level: f32) -> String {
        let filled = (level * 5.0) as usize;
        let bar: String = (0..5).map(|i| if i < filled { '▊' } else { '░' }).collect();
        format!("{} {:.0}%", bar, level * 100.0)
    }
}

impl Default for InstrumentPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for InstrumentPane {
    fn id(&self) -> &'static str {
        "instrument"
    }

    fn handle_action(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
        match action {
            "quit" => Action::Quit,
            "next" => Action::Instrument(InstrumentAction::SelectNext),
            "prev" => Action::Instrument(InstrumentAction::SelectPrev),
            "goto_top" => Action::Instrument(InstrumentAction::SelectFirst),
            "goto_bottom" => Action::Instrument(InstrumentAction::SelectLast),
            "add" => Action::Nav(NavAction::SwitchPane("add")),
            "delete" => {
                if let Some(instrument) = state.instruments.selected_instrument() {
                    Action::Instrument(InstrumentAction::Delete(instrument.id))
                } else {
                    Action::None
                }
            }
            "edit" => {
                if let Some(instrument) = state.instruments.selected_instrument() {
                    Action::Instrument(InstrumentAction::Edit(instrument.id))
                } else {
                    Action::None
                }
            }
            "save" => Action::Session(SessionAction::Save),
            "load" => Action::Session(SessionAction::Load),

            // Piano layer actions
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
                            return Action::Instrument(InstrumentAction::PlayNotes(pitches, 100));
                        }
                    }
                }
                Action::None
            }

            // Pad layer actions
            "pad:escape" => {
                self.pad_keyboard.deactivate();
                Action::ExitPerformanceMode
            }
            "pad:key" => {
                if let KeyCode::Char(c) = event.key {
                    let c = translate_key(c, state.keyboard_layout);
                    if let Some(pad_idx) = self.pad_keyboard.key_to_pad(c) {
                        return Action::Instrument(InstrumentAction::PlayDrumPad(pad_idx));
                    }
                }
                Action::None
            }

            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Instruments ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let content_x = inner.x + 1;
        let content_y = inner.y + 1;

        Paragraph::new(Line::from(Span::styled(
            "Instruments:",
            ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold()),
        ))).render(RatatuiRect::new(content_x, content_y, inner.width.saturating_sub(2), 1), buf);

        let list_y = content_y + 2;
        let max_visible = ((inner.height.saturating_sub(7)) as usize).max(3);

        if state.instruments.instruments.is_empty() {
            Paragraph::new(Line::from(Span::styled(
                "(no instruments — press 'a' to add)",
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ))).render(RatatuiRect::new(content_x + 2, list_y, inner.width.saturating_sub(4), 1), buf);
        }

        let scroll_offset = state.instruments.selected
            .map(|s| if s >= max_visible { s - max_visible + 1 } else { 0 })
            .unwrap_or(0);
        let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));

        for (i, instrument) in state.instruments.instruments.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let is_selected = state.instruments.selected == Some(i);

            // Selection indicator
            if is_selected {
                if let Some(cell) = buf.cell_mut((content_x, y)) {
                    cell.set_char('>').set_style(
                        ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                    );
                }
            }

            let mk_style = |fg: Color| -> ratatui::style::Style {
                if is_selected {
                    ratatui::style::Style::from(Style::new().fg(fg).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(fg))
                }
            };

            // Build row as a Line with multiple spans
            let name_str = format!("{:14}", &instrument.name[..instrument.name.len().min(14)]);
            let source_str = format!(" {:10}", instrument.source.name());
            let filter_str = format!(" {:12}", Self::format_filter(instrument));
            let eq_str = format!(" {:4}", Self::format_eq(instrument));
            let fx_raw = Self::format_effects(instrument);
            let fx_str = format!(" {:18}", &fx_raw[..fx_raw.len().min(18)]);
            let level_str = format!(" {}", Self::format_level(instrument.level));

            let source_c = source_color(instrument.source);

            let line = Line::from(vec![
                Span::styled(name_str, mk_style(Color::WHITE)),
                Span::styled(source_str, mk_style(source_c)),
                Span::styled(filter_str, mk_style(Color::FILTER_COLOR)),
                Span::styled(eq_str, mk_style(Color::EQ_COLOR)),
                Span::styled(fx_str, mk_style(Color::FX_COLOR)),
                Span::styled(level_str, mk_style(Color::LIME)),
            ]);
            let line_width = inner.width.saturating_sub(3);
            Paragraph::new(line).render(
                RatatuiRect::new(content_x + 2, y, line_width, 1), buf,
            );

            // Fill rest of line with selection bg
            if is_selected {
                let fill_start = content_x + 2 + line_width;
                let fill_end = inner.x + inner.width;
                for x in fill_start..fill_end {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ').set_style(sel_bg);
                    }
                }
            }
        }

        // Scroll indicators
        let scroll_style = ratatui::style::Style::from(Style::new().fg(Color::ORANGE));
        if scroll_offset > 0 {
            Paragraph::new(Line::from(Span::styled("...", scroll_style)))
                .render(RatatuiRect::new(rect.x + rect.width - 5, list_y, 3, 1), buf);
        }
        if scroll_offset + max_visible < state.instruments.instruments.len() {
            Paragraph::new(Line::from(Span::styled("...", scroll_style)))
                .render(RatatuiRect::new(rect.x + rect.width - 5, list_y + max_visible as u16 - 1, 3, 1), buf);
        }

        // Piano/Pad mode indicator
        if self.pad_keyboard.is_active() {
            let pad_str = self.pad_keyboard.status_label();
            let pad_x = rect.x + rect.width - pad_str.len() as u16 - 1;
            Paragraph::new(Line::from(Span::styled(
                pad_str.clone(),
                ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::KIT_COLOR)),
            ))).render(RatatuiRect::new(pad_x, rect.y, pad_str.len() as u16, 1), buf);
        } else if self.piano.is_active() {
            let piano_str = self.piano.status_label();
            let piano_x = rect.x + rect.width - piano_str.len() as u16 - 1;
            Paragraph::new(Line::from(Span::styled(
                piano_str.clone(),
                ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::PINK)),
            ))).render(RatatuiRect::new(piano_x, rect.y, piano_str.len() as u16, 1), buf);
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        let help_text = if self.pad_keyboard.is_active() {
            "R T Y U / F G H J / V B N M: trigger pads | /: cycle | Esc: exit"
        } else if self.piano.is_active() {
            "Play keys | [/]: octave | \u{2191}/\u{2193}: select instrument | /: cycle | Esc: exit"
        } else {
            "a: add | d: delete | Enter: edit | /: piano | w: save | o: load"
        };
        Paragraph::new(Line::from(Span::styled(
            help_text,
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(RatatuiRect::new(content_x, help_y, inner.width.saturating_sub(2), 1), buf);
    }

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action {
        let rect = center_rect(area, 97, 29);
        let inner_x = rect.x + 2;
        let inner_y = rect.y + 2;
        let content_y = inner_y + 1;
        let list_y = content_y + 2;
        let inner_height = rect.height.saturating_sub(4);
        let max_visible = ((inner_height.saturating_sub(7)) as usize).max(3);

        let scroll_offset = state.instruments.selected
            .map(|s| if s >= max_visible { s - max_visible + 1 } else { 0 })
            .unwrap_or(0);

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = event.column;
                let row = event.row;
                // Click on instrument list
                if col >= inner_x && row >= list_y && row < list_y + max_visible as u16 {
                    let clicked_idx = scroll_offset + (row - list_y) as usize;
                    if clicked_idx < state.instruments.instruments.len() {
                        return Action::Instrument(InstrumentAction::Select(clicked_idx));
                    }
                }
                Action::None
            }
            MouseEventKind::ScrollUp => Action::Instrument(InstrumentAction::SelectPrev),
            MouseEventKind::ScrollDown => Action::Instrument(InstrumentAction::SelectNext),
            _ => Action::None,
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn toggle_performance_mode(&mut self, state: &AppState) -> ToggleResult {
        if self.pad_keyboard.is_active() {
            self.pad_keyboard.deactivate();
            ToggleResult::Deactivated
        } else if self.piano.is_active() {
            self.piano.handle_escape();
            if self.piano.is_active() {
                ToggleResult::CycledLayout
            } else {
                ToggleResult::Deactivated
            }
        } else if state.instruments.selected_instrument()
            .map_or(false, |s| s.source.is_kit())
        {
            self.pad_keyboard.activate();
            ToggleResult::ActivatedPad
        } else {
            self.piano.activate();
            ToggleResult::ActivatedPiano
        }
    }

    fn activate_piano(&mut self) {
        if !self.piano.is_active() { self.piano.activate(); }
        self.pad_keyboard.deactivate();
    }

    fn activate_pad(&mut self) {
        if !self.pad_keyboard.is_active() { self.pad_keyboard.activate(); }
        self.piano.deactivate();
    }

    fn deactivate_performance(&mut self) {
        self.piano.deactivate();
        self.pad_keyboard.deactivate();
    }

    fn supports_performance_mode(&self) -> bool { true }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, SourceType};
    use crate::ui::{InputEvent, KeyCode, Modifiers};

    fn dummy_event() -> InputEvent {
        InputEvent::new(KeyCode::Char('x'), Modifiers::default())
    }

    #[test]
    fn delete_returns_selected_instrument_id() {
        let mut state = AppState::new();
        let id = state.add_instrument(SourceType::Saw);
        let mut pane = InstrumentPane::new(Keymap::new());

        let action = pane.handle_action("delete", &dummy_event(), &state);
        match action {
            Action::Instrument(InstrumentAction::Delete(got)) => assert_eq!(got, id),
            _ => panic!("Expected InstrumentAction::Delete"),
        }
    }

    #[test]
    fn edit_returns_selected_instrument_id() {
        let mut state = AppState::new();
        let id = state.add_instrument(SourceType::Sin);
        let mut pane = InstrumentPane::new(Keymap::new());

        let action = pane.handle_action("edit", &dummy_event(), &state);
        match action {
            Action::Instrument(InstrumentAction::Edit(got)) => assert_eq!(got, id),
            _ => panic!("Expected InstrumentAction::Edit"),
        }
    }

    #[test]
    fn add_navigates_to_add_pane() {
        let state = AppState::new();
        let mut pane = InstrumentPane::new(Keymap::new());

        let action = pane.handle_action("add", &dummy_event(), &state);
        match action {
            Action::Nav(NavAction::SwitchPane(id)) => assert_eq!(id, "add"),
            _ => panic!("Expected SwitchPane(add)"),
        }
    }

    #[test]
    fn next_prev_return_select_actions() {
        let state = AppState::new();
        let mut pane = InstrumentPane::new(Keymap::new());

        let action = pane.handle_action("next", &dummy_event(), &state);
        assert!(matches!(action, Action::Instrument(InstrumentAction::SelectNext)));

        let action = pane.handle_action("prev", &dummy_event(), &state);
        assert!(matches!(action, Action::Instrument(InstrumentAction::SelectPrev)));
    }
}
