use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::action::{Action, MidiAction};
use crate::state::AppState;
use crate::ui::{Color, InputEvent, Keymap, Pane, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    Ports,
    CcMappings,
    Settings,
}

pub struct MidiSettingsPane {
    keymap: Keymap,
    section: Section,
    port_cursor: usize,
    mapping_cursor: usize,
}

impl MidiSettingsPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            section: Section::Ports,
            port_cursor: 0,
            mapping_cursor: 0,
        }
    }
}

impl Pane for MidiSettingsPane {
    fn id(&self) -> &'static str {
        "midi_settings"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        match action {
            "switch_section" => {
                self.section = match self.section {
                    Section::Ports => Section::CcMappings,
                    Section::CcMappings => Section::Settings,
                    Section::Settings => Section::Ports,
                };
                Action::None
            }
            "up" => {
                match self.section {
                    Section::Ports => {
                        self.port_cursor = self.port_cursor.saturating_sub(1);
                    }
                    Section::CcMappings => {
                        self.mapping_cursor = self.mapping_cursor.saturating_sub(1);
                    }
                    Section::Settings => {}
                }
                Action::None
            }
            "down" => {
                match self.section {
                    Section::Ports => {
                        let max = state.midi_port_names.len().saturating_sub(1);
                        self.port_cursor = (self.port_cursor + 1).min(max);
                    }
                    Section::CcMappings => {
                        let max = state.session.midi_recording.cc_mappings.len().saturating_sub(1);
                        self.mapping_cursor = (self.mapping_cursor + 1).min(max);
                    }
                    Section::Settings => {}
                }
                Action::None
            }
            "connect" => {
                if self.section == Section::Ports && !state.midi_port_names.is_empty() {
                    Action::Midi(MidiAction::ConnectPort(self.port_cursor))
                } else {
                    Action::None
                }
            }
            "disconnect" => {
                Action::Midi(MidiAction::DisconnectPort)
            }
            "remove_mapping" => {
                if self.section == Section::CcMappings {
                    let mappings = &state.session.midi_recording.cc_mappings;
                    if let Some(m) = mappings.get(self.mapping_cursor) {
                        let cc = m.cc_number;
                        let ch = m.channel;
                        return Action::Midi(MidiAction::RemoveCcMapping { cc, channel: ch });
                    }
                }
                Action::None
            }
            "toggle_passthrough" => {
                Action::Midi(MidiAction::ToggleNotePassthrough)
            }
            "set_channel_all" => {
                Action::Midi(MidiAction::SetChannelFilter(None))
            }
            "set_live_instrument" => {
                if let Some(inst) = state.instruments.selected_instrument() {
                    Action::Midi(MidiAction::SetLiveInputInstrument(Some(inst.id)))
                } else {
                    Action::None
                }
            }
            "clear_live_instrument" => {
                Action::Midi(MidiAction::SetLiveInputInstrument(None))
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" MIDI Settings ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 20 {
            return;
        }

        let section_style = |s: Section| {
            if s == self.section {
                ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold())
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            }
        };
        let normal = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
        let dim = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let highlight = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());

        let mut y = inner.y;
        let x = inner.x + 1;
        let w = inner.width.saturating_sub(2);

        // Section: Ports
        let port_header = Line::from(vec![
            Span::styled(" Ports ", section_style(Section::Ports)),
            Span::styled(
                if let Some(ref name) = state.midi_connected_port {
                    format!("  [Connected: {}]", name)
                } else {
                    "  [Not connected]".to_string()
                },
                dim,
            ),
        ]);
        Paragraph::new(port_header).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        if self.section == Section::Ports {
            if state.midi_port_names.is_empty() {
                Paragraph::new(Line::from(Span::styled("  (no MIDI ports found)", dim)))
                    .render(RatatuiRect::new(x, y, w, 1), buf);
                y += 1;
            } else {
                for (i, name) in state.midi_port_names.iter().enumerate() {
                    if y >= inner.y + inner.height { break; }
                    let is_connected = state.midi_connected_port.as_deref() == Some(name);
                    let prefix = if is_connected { " * " } else { "   " };
                    let text = format!("{}{}", prefix, name);
                    let style = if i == self.port_cursor { highlight } else { normal };
                    Paragraph::new(Line::from(Span::styled(text, style)))
                        .render(RatatuiRect::new(x, y, w, 1), buf);
                    y += 1;
                }
            }
        }
        y += 1;

        // Section: CC Mappings
        if y >= inner.y + inner.height { return; }
        let mapping_header = Line::from(Span::styled(
            format!(" CC Mappings ({})", state.session.midi_recording.cc_mappings.len()),
            section_style(Section::CcMappings),
        ));
        Paragraph::new(mapping_header).render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        if self.section == Section::CcMappings {
            if state.session.midi_recording.cc_mappings.is_empty() {
                if y < inner.y + inner.height {
                    Paragraph::new(Line::from(Span::styled("  (no CC mappings)", dim)))
                        .render(RatatuiRect::new(x, y, w, 1), buf);
                    y += 1;
                }
            } else {
                for (i, mapping) in state.session.midi_recording.cc_mappings.iter().enumerate() {
                    if y >= inner.y + inner.height { break; }
                    let ch_str = match mapping.channel {
                        Some(ch) => format!("ch{}", ch + 1),
                        None => "any".to_string(),
                    };
                    let text = format!(
                        "  CC{:<3} {} -> {}",
                        mapping.cc_number, ch_str, mapping.target.name()
                    );
                    let style = if i == self.mapping_cursor { highlight } else { normal };
                    Paragraph::new(Line::from(Span::styled(text, style)))
                        .render(RatatuiRect::new(x, y, w, 1), buf);
                    y += 1;
                }
            }
        }
        y += 1;

        // Section: Settings
        if y >= inner.y + inner.height { return; }
        Paragraph::new(Line::from(Span::styled(" Settings", section_style(Section::Settings))))
            .render(RatatuiRect::new(x, y, w, 1), buf);
        y += 1;

        if self.section == Section::Settings {
            let settings = [
                format!("  Note passthrough: {}", if state.session.midi_recording.note_passthrough { "ON" } else { "OFF" }),
                format!("  Channel filter: {}", match state.session.midi_recording.channel_filter {
                    Some(ch) => format!("Ch {}", ch + 1),
                    None => "All".to_string(),
                }),
                format!("  Live input instrument: {}", match state.session.midi_recording.live_input_instrument {
                    Some(id) => {
                        state.instruments.instruments.iter()
                            .find(|i| i.id == id)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|| format!("#{}", id))
                    }
                    None => "(selected)".to_string(),
                }),
            ];

            for line in &settings {
                if y >= inner.y + inner.height { break; }
                Paragraph::new(Line::from(Span::styled(line.as_str(), normal)))
                    .render(RatatuiRect::new(x, y, w, 1), buf);
                y += 1;
            }
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_settings_pane_id() {
        let pane = MidiSettingsPane::new(Keymap::new());
        assert_eq!(pane.id(), "midi_settings");
    }
}
