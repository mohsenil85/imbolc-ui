use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, SourceType};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, NavAction, InstrumentAction, SessionAction, Color, InputEvent, KeyCode, Keymap, PadKeyboard, Pane, PianoKeyboard, Style};

fn source_color(source: SourceType) -> Color {
    match source {
        SourceType::Saw => Color::OSC_COLOR,
        SourceType::Sin => Color::OSC_COLOR,
        SourceType::Sqr => Color::OSC_COLOR,
        SourceType::Tri => Color::OSC_COLOR,
        SourceType::AudioIn => Color::AUDIO_IN_COLOR,
        SourceType::Sample => Color::SAMPLE_COLOR,
        SourceType::Kit => Color::KIT_COLOR,
        SourceType::BusIn => Color::BUS_IN_COLOR,
        SourceType::Custom(_) => Color::CUSTOM_COLOR,
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

    fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action {
        // Pad keyboard mode: letter keys trigger drum pads
        if self.pad_keyboard.is_active() {
            match event.key {
                KeyCode::Up => {
                    return Action::Instrument(InstrumentAction::SelectPrev);
                }
                KeyCode::Down => {
                    return Action::Instrument(InstrumentAction::SelectNext);
                }
                KeyCode::Char(c) => {
                    if let Some(pad_idx) = self.pad_keyboard.key_to_pad(c) {
                        return Action::Instrument(InstrumentAction::PlayDrumPad(pad_idx));
                    }
                    return Action::None;
                }
                _ => return Action::None,
            }
        }

        // Piano mode: letter keys play notes
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
                    return Action::Instrument(InstrumentAction::SelectPrev);
                }
                KeyCode::Down => {
                    return Action::Instrument(InstrumentAction::SelectNext);
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

        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("next") => Action::Instrument(InstrumentAction::SelectNext),
            Some("prev") => Action::Instrument(InstrumentAction::SelectPrev),
            Some("goto_top") => Action::Instrument(InstrumentAction::SelectFirst),
            Some("goto_bottom") => Action::Instrument(InstrumentAction::SelectLast),
            Some("add") => Action::Nav(NavAction::SwitchPane("add")),
            Some("delete") => {
                if let Some(instrument) = state.instruments.selected_instrument() {
                    Action::Instrument(InstrumentAction::Delete(instrument.id))
                } else {
                    Action::None
                }
            }
            Some("edit") => {
                if let Some(instrument) = state.instruments.selected_instrument() {
                    Action::Instrument(InstrumentAction::Edit(instrument.id))
                } else {
                    Action::None
                }
            }
            Some("save") => Action::Session(SessionAction::Save),
            Some("load") => Action::Session(SessionAction::Load),
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
            let osc_str = format!(" {:10}", instrument.source.name());
            let filter_str = format!(" {:12}", Self::format_filter(instrument));
            let fx_raw = Self::format_effects(instrument);
            let fx_str = format!(" {:18}", &fx_raw[..fx_raw.len().min(18)]);
            let level_str = format!(" {}", Self::format_level(instrument.level));

            let osc_c = source_color(instrument.source);

            let line = Line::from(vec![
                Span::styled(name_str, mk_style(Color::WHITE)),
                Span::styled(osc_str, mk_style(osc_c)),
                Span::styled(filter_str, mk_style(Color::FILTER_COLOR)),
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

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn wants_exclusive_input(&self) -> bool {
        self.piano.is_active() || self.pad_keyboard.is_active()
    }

    fn toggle_piano_mode(&mut self, state: &AppState) -> bool {
        if self.pad_keyboard.is_active() {
            self.pad_keyboard.handle_escape();
        } else if self.piano.is_active() {
            self.piano.handle_escape();
        } else if state.instruments.selected_instrument()
            .map_or(false, |s| s.source.is_kit())
        {
            self.pad_keyboard.activate();
        } else {
            self.piano.activate();
        }
        true
    }

    fn exit_piano_mode(&mut self) -> bool {
        if self.piano.is_active() {
            self.piano.deactivate();
            true
        } else if self.pad_keyboard.is_active() {
            self.pad_keyboard.deactivate();
            true
        } else {
            false
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
