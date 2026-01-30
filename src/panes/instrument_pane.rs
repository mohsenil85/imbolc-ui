use std::any::Any;

use crate::state::{AppState, OscType};
use crate::ui::{Action, NavAction, InstrumentAction, SessionAction, Color, Graphics, InputEvent, KeyCode, Keymap, PadKeyboard, Pane, PianoKeyboard, Rect, Style};

fn osc_color(osc: OscType) -> Color {
    match osc {
        OscType::Saw => Color::OSC_COLOR,
        OscType::Sin => Color::OSC_COLOR,
        OscType::Sqr => Color::OSC_COLOR,
        OscType::Tri => Color::OSC_COLOR,
        OscType::AudioIn => Color::AUDIO_IN_COLOR,
        OscType::Sample => Color::SAMPLE_COLOR,
        OscType::Kit => Color::KIT_COLOR,
        OscType::BusIn => Color::BUS_IN_COLOR,
        OscType::Custom(_) => Color::CUSTOM_COLOR,
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
                KeyCode::Char('/') | KeyCode::Escape => {
                    self.pad_keyboard.handle_escape();
                    return Action::None;
                }
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
                KeyCode::Char('/') => {
                    self.piano.handle_escape();
                    return Action::None;
                }
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
            Some("piano_mode") => {
                // Activate pad keyboard for drum machines, piano for everything else
                if state.instruments.selected_instrument()
                    .map_or(false, |s| s.source.is_kit())
                {
                    self.pad_keyboard.activate();
                } else {
                    self.piano.activate();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, state: &AppState) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::CYAN));
        g.draw_box(rect, Some(" Instruments "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        g.set_style(Style::new().fg(Color::CYAN).bold());
        g.put_str(content_x, content_y, "Instruments:");

        let list_y = content_y + 2;
        let max_visible = ((rect.height - 8) as usize).max(3);

        if state.instruments.instruments.is_empty() {
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            g.put_str(content_x + 2, list_y, "(no instruments — press 'a' to add)");
        }

        let scroll_offset = state.instruments.selected
            .map(|s| if s >= max_visible { s - max_visible + 1 } else { 0 })
            .unwrap_or(0);

        for (i, instrument) in state.instruments.instruments.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;
            let is_selected = state.instruments.selected == Some(i);

            // Selection indicator
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, ">");
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x, y, " ");
            }

            // Instrument name
            let name_str = format!("{:14}", &instrument.name[..instrument.name.len().min(14)]);
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::WHITE));
            }
            g.put_str(content_x + 2, y, &name_str);

            // Osc type
            let osc_c = osc_color(instrument.source);
            if is_selected {
                g.set_style(Style::new().fg(osc_c).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(osc_c));
            }
            g.put_str(content_x + 17, y, &format!("{:10}", instrument.source.name()));

            // Filter
            let filter_str = Self::format_filter(instrument);
            if is_selected {
                g.set_style(Style::new().fg(Color::FILTER_COLOR).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::FILTER_COLOR));
            }
            g.put_str(content_x + 28, y, &format!("{:12}", filter_str));

            // Effects
            let fx_str = Self::format_effects(instrument);
            if is_selected {
                g.set_style(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::FX_COLOR));
            }
            g.put_str(content_x + 41, y, &format!("{:18}", &fx_str[..fx_str.len().min(18)]));

            // Level bar
            let level_str = Self::format_level(instrument.level);
            if is_selected {
                g.set_style(Style::new().fg(Color::LIME).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::LIME));
            }
            g.put_str(content_x + 60, y, &level_str);

            // Clear to end if selected
            if is_selected {
                g.set_style(Style::new().bg(Color::SELECTION_BG));
                let line_end = content_x + 60 + level_str.len() as u16;
                for x in line_end..(rect.x + rect.width - 2) {
                    g.put_char(x, y, ' ');
                }
            }
        }

        // Scroll indicators
        if scroll_offset > 0 {
            g.set_style(Style::new().fg(Color::ORANGE));
            g.put_str(rect.x + rect.width - 4, list_y, "...");
        }
        if scroll_offset + max_visible < state.instruments.instruments.len() {
            g.set_style(Style::new().fg(Color::ORANGE));
            g.put_str(rect.x + rect.width - 4, list_y + max_visible as u16 - 1, "...");
        }

        // Piano/Pad mode indicator
        if self.pad_keyboard.is_active() {
            g.set_style(Style::new().fg(Color::BLACK).bg(Color::KIT_COLOR));
            let pad_str = self.pad_keyboard.status_label();
            let pad_x = rect.x + rect.width - pad_str.len() as u16 - 1;
            g.put_str(pad_x, rect.y, &pad_str);
        } else if self.piano.is_active() {
            g.set_style(Style::new().fg(Color::BLACK).bg(Color::PINK));
            let piano_str = self.piano.status_label();
            let piano_x = rect.x + rect.width - piano_str.len() as u16 - 1;
            g.put_str(piano_x, rect.y, &piano_str);
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        if self.pad_keyboard.is_active() {
            g.put_str(content_x, help_y, "R T Y U / F G H J / V B N M: trigger pads | /: exit pad mode");
        } else if self.piano.is_active() {
            g.put_str(content_x, help_y, "Play keys | [/]: octave | ↑/↓: select instrument | /: cycle layout/exit");
        } else {
            g.put_str(content_x, help_y, "a: add | d: delete | Enter: edit | /: piano | w: save | o: load");
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn wants_exclusive_input(&self) -> bool {
        self.piano.is_active() || self.pad_keyboard.is_active()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
