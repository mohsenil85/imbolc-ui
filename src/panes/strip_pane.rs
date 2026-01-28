use std::any::Any;

use crate::state::{EffectType, FilterType, OscType, StripId, StripState};
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style};

fn osc_color(osc: OscType) -> Color {
    match osc {
        OscType::Saw => Color::OSC_COLOR,
        OscType::Sin => Color::OSC_COLOR,
        OscType::Sqr => Color::OSC_COLOR,
        OscType::Tri => Color::OSC_COLOR,
        OscType::AudioIn => Color::AUDIO_IN_COLOR,
        OscType::Sampler => Color::SAMPLER_COLOR,
    }
}

/// Piano keyboard layout starting note
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PianoLayout {
    C, // Standard C-based layout
    A, // A-based layout (natural minor friendly)
}

pub struct StripPane {
    keymap: Keymap,
    state: StripState,
    // Piano keyboard mode
    piano_mode: bool,
    piano_octave: i8,
    piano_layout: PianoLayout,
}

impl StripPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind('q', "quit", "Quit the application")
                .bind_key(KeyCode::Down, "next", "Next strip")
                .bind_key(KeyCode::Up, "prev", "Previous strip")
                .bind_key(KeyCode::Home, "goto_top", "Go to top")
                .bind_key(KeyCode::End, "goto_bottom", "Go to bottom")
                .bind('a', "add", "Add strip")
                .bind('d', "delete", "Delete strip")
                .bind_key(KeyCode::Enter, "edit", "Edit strip")
                .bind('w', "save", "Save")
                .bind('o', "load", "Load")
                .bind('/', "piano_mode", "Toggle piano keyboard mode"),
            state: StripState::new(),
            piano_mode: false,
            piano_octave: 4,
            piano_layout: PianoLayout::C,
        }
    }

    /// Map a keyboard character to a MIDI note offset for C layout
    fn key_to_offset_c(key: char) -> Option<u8> {
        match key {
            'a' => Some(0),   // C
            's' => Some(2),   // D
            'd' => Some(4),   // E
            'f' => Some(5),   // F
            'g' => Some(7),   // G
            'h' => Some(9),   // A
            'j' => Some(11),  // B
            'w' => Some(1),   // C#
            'e' => Some(3),   // D#
            't' => Some(6),   // F#
            'y' => Some(8),   // G#
            'u' => Some(10),  // A#
            'k' => Some(12),  // C (octave up)
            'l' => Some(14),  // D
            ';' => Some(16),  // E
            'o' => Some(13),  // C#
            'p' => Some(15),  // D#
            _ => None,
        }
    }

    /// Map a keyboard character to a MIDI note offset for A layout
    fn key_to_offset_a(key: char) -> Option<u8> {
        match key {
            'a' => Some(0),   // A
            's' => Some(2),   // B
            'd' => Some(3),   // C
            'f' => Some(5),   // D
            'g' => Some(7),   // E
            'h' => Some(8),   // F
            'j' => Some(10),  // G
            'w' => Some(1),   // A#
            'e' => Some(4),   // C#
            't' => Some(6),   // D#
            'y' => Some(9),   // F#
            'u' => Some(11),  // G#
            'k' => Some(12),  // A (octave up)
            'l' => Some(14),  // B
            ';' => Some(15),  // C
            'o' => Some(13),  // A#
            'p' => Some(16),  // C#
            _ => None,
        }
    }

    /// Convert a key to a MIDI pitch using current octave and layout
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

    pub fn state(&self) -> &StripState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut StripState {
        &mut self.state
    }

    pub fn set_state(&mut self, state: StripState) {
        self.state = state;
    }

    fn format_filter(strip: &crate::state::strip::Strip) -> String {
        match &strip.filter {
            Some(f) => format!("[{}]", f.filter_type.name()),
            None => "---".to_string(),
        }
    }

    fn format_effects(strip: &crate::state::strip::Strip) -> String {
        if strip.effects.is_empty() {
            return "---".to_string();
        }
        strip.effects.iter()
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

impl Default for StripPane {
    fn default() -> Self {
        Self::new()
    }
}

impl StripPane {
    pub fn piano_mode(&self) -> bool {
        self.piano_mode
    }
}

impl Pane for StripPane {
    fn id(&self) -> &'static str {
        "strip"
    }

    fn handle_input(&mut self, event: InputEvent) -> Action {
        // Piano mode: letter keys play notes
        if self.piano_mode {
            match event.key {
                KeyCode::Char('/') => {
                    // Cycle: C -> A -> exit
                    match self.piano_layout {
                        PianoLayout::C => self.piano_layout = PianoLayout::A,
                        PianoLayout::A => self.piano_mode = false,
                    }
                    return Action::None;
                }
                KeyCode::Char('[') => {
                    if self.piano_octave > -1 {
                        self.piano_octave -= 1;
                    }
                    return Action::None;
                }
                KeyCode::Char(']') => {
                    if self.piano_octave < 9 {
                        self.piano_octave += 1;
                    }
                    return Action::None;
                }
                KeyCode::Up => {
                    self.state.select_prev();
                    return Action::None;
                }
                KeyCode::Down => {
                    self.state.select_next();
                    return Action::None;
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

        match self.keymap.lookup(&event) {
            Some("quit") => Action::Quit,
            Some("next") => {
                self.state.select_next();
                Action::None
            }
            Some("prev") => {
                self.state.select_prev();
                Action::None
            }
            Some("goto_top") => {
                if !self.state.strips.is_empty() {
                    self.state.selected = Some(0);
                }
                Action::None
            }
            Some("goto_bottom") => {
                if !self.state.strips.is_empty() {
                    self.state.selected = Some(self.state.strips.len() - 1);
                }
                Action::None
            }
            Some("add") => Action::SwitchPane("add"),
            Some("delete") => {
                if let Some(strip) = self.state.selected_strip() {
                    Action::DeleteStrip(strip.id)
                } else {
                    Action::None
                }
            }
            Some("edit") => {
                if let Some(strip) = self.state.selected_strip() {
                    Action::EditStrip(strip.id)
                } else {
                    Action::None
                }
            }
            Some("save") => Action::SaveRack,
            Some("load") => Action::LoadRack,
            Some("piano_mode") => {
                self.piano_mode = true;
                self.piano_layout = PianoLayout::C;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::CYAN));
        g.draw_box(rect, Some(" Strips "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        g.set_style(Style::new().fg(Color::CYAN).bold());
        g.put_str(content_x, content_y, "Instrument Strips:");

        let list_y = content_y + 2;
        let max_visible = ((rect.height - 8) as usize).max(3);

        if self.state.strips.is_empty() {
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            g.put_str(content_x + 2, list_y, "(no strips — press 'a' to add)");
        }

        let scroll_offset = self.state.selected
            .map(|s| if s >= max_visible { s - max_visible + 1 } else { 0 })
            .unwrap_or(0);

        for (i, strip) in self.state.strips.iter().enumerate().skip(scroll_offset) {
            let row = i - scroll_offset;
            if row >= max_visible {
                break;
            }
            let y = list_y + row as u16;
            let is_selected = self.state.selected == Some(i);

            // Selection indicator
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold());
                g.put_str(content_x, y, ">");
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(content_x, y, " ");
            }

            // Strip name
            let name_str = format!("{:14}", &strip.name[..strip.name.len().min(14)]);
            if is_selected {
                g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::WHITE));
            }
            g.put_str(content_x + 2, y, &name_str);

            // Osc type
            let osc_c = osc_color(strip.source);
            if is_selected {
                g.set_style(Style::new().fg(osc_c).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(osc_c));
            }
            g.put_str(content_x + 17, y, &format!("{:10}", strip.source.name()));

            // Filter
            let filter_str = Self::format_filter(strip);
            if is_selected {
                g.set_style(Style::new().fg(Color::FILTER_COLOR).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::FILTER_COLOR));
            }
            g.put_str(content_x + 28, y, &format!("{:12}", filter_str));

            // Effects
            let fx_str = Self::format_effects(strip);
            if is_selected {
                g.set_style(Style::new().fg(Color::FX_COLOR).bg(Color::SELECTION_BG));
            } else {
                g.set_style(Style::new().fg(Color::FX_COLOR));
            }
            g.put_str(content_x + 41, y, &format!("{:18}", &fx_str[..fx_str.len().min(18)]));

            // Level bar
            let level_str = Self::format_level(strip.level);
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
        if scroll_offset + max_visible < self.state.strips.len() {
            g.set_style(Style::new().fg(Color::ORANGE));
            g.put_str(rect.x + rect.width - 4, list_y + max_visible as u16 - 1, "...");
        }

        // Piano mode indicator
        if self.piano_mode {
            g.set_style(Style::new().fg(Color::BLACK).bg(Color::PINK));
            let layout_char = match self.piano_layout {
                PianoLayout::C => 'C',
                PianoLayout::A => 'A',
            };
            let piano_str = format!(" PIANO {}{} ", layout_char, self.piano_octave);
            let piano_x = rect.x + rect.width - piano_str.len() as u16 - 1;
            g.put_str(piano_x, rect.y, &piano_str);
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        if self.piano_mode {
            g.put_str(content_x, help_y, "Play keys | [/]: octave | ↑/↓: select strip | /: cycle layout/exit");
        } else {
            g.put_str(content_x, help_y, "a: add | d: delete | Enter: edit | /: piano | w: save | o: load");
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn wants_exclusive_input(&self) -> bool {
        self.piano_mode
    }

    fn receive_action(&mut self, action: &Action) -> bool {
        match action {
            Action::AddStrip(osc_type) => {
                self.state.add_strip(*osc_type);
                true
            }
            _ => false,
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
