use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, Pane, Style};

/// Waveform display characters (8 levels)
const WAVEFORM_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Color a waveform row by its distance from center (0.0=center, 1.0=edge)
fn waveform_color(frac: f32) -> Color {
    if frac > 0.85 {
        Color::new(220, 40, 40)   // red
    } else if frac > 0.7 {
        Color::new(220, 120, 30)  // orange
    } else if frac > 0.5 {
        Color::new(200, 200, 40)  // yellow
    } else {
        Color::new(60, 200, 80)   // green
    }
}

pub struct WaveformPane {
    keymap: Keymap,
    /// Waveform peaks from a recorded audio file
    pub recorded_waveform: Option<Vec<f32>>,
    /// Live waveform from audio input
    pub audio_in_waveform: Option<Vec<f32>>,
}

impl WaveformPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            recorded_waveform: None,
            audio_in_waveform: None,
        }
    }
}

impl Default for WaveformPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for WaveformPane {
    fn id(&self) -> &'static str {
        "waveform"
    }

    fn handle_action(&mut self, _action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let is_recorded = self.recorded_waveform.is_some();
        let waveform = self.recorded_waveform.as_deref()
            .or(self.audio_in_waveform.as_deref())
            .unwrap_or(&[]);

        let rect = center_rect(area, 97, 29);

        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + 1;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(2);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        // Border with instrument label
        let title = if is_recorded {
            " Recorded Waveform ".to_string()
        } else if let Some(inst) = state.instruments.selected_instrument() {
            format!(" Audio Input: {} ", inst.name)
        } else {
            " Audio Input ".to_string()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::AUDIO_IN_COLOR)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::AUDIO_IN_COLOR)));
        block.render(rect, buf);

        // Header: transport info
        let piano_roll = &state.session.piano_roll;
        let header_y = rect.y + 1;
        let play_icon = if piano_roll.playing { "||" } else { "> " };
        let header_text = format!(
            " BPM:{:.0}  {}  Waveform Display",
            piano_roll.bpm,
            play_icon,
        );
        Paragraph::new(Line::from(Span::styled(
            header_text,
            ratatui::style::Style::from(Style::new().fg(Color::WHITE)),
        ))).render(RatatuiRect::new(rect.x + 1, header_y, rect.width.saturating_sub(2), 1), buf);

        // Waveform display area
        let center_y = grid_y + grid_height / 2;
        let half_height = (grid_height / 2) as f32;

        // Draw center line
        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        for x in 0..grid_width {
            if let Some(cell) = buf.cell_mut((grid_x + x, center_y)) {
                cell.set_char('─').set_style(dark_gray);
            }
        }

        // Draw waveform with amplitude-based coloring
        let waveform_len = waveform.len();
        let max_half = (grid_height / 2).max(1);
        for col in 0..grid_width as usize {
            let sample_idx = if waveform_len > 0 {
                (col * waveform_len / grid_width as usize).min(waveform_len - 1)
            } else {
                0
            };

            let amplitude = if sample_idx < waveform_len {
                waveform[sample_idx].abs().min(1.0)
            } else {
                0.0
            };

            let bar_height = (amplitude * half_height) as u16;

            // Upper half (above center)
            for dy in 0..bar_height.min(max_half) {
                let y = center_y.saturating_sub(dy + 1);
                let frac = (dy + 1) as f32 / max_half as f32;
                let color = waveform_color(frac);
                let style = ratatui::style::Style::from(Style::new().fg(color));
                let char_idx = if dy + 1 == bar_height { ((amplitude * 7.0) as usize).min(7) } else { 7 };
                if let Some(cell) = buf.cell_mut((grid_x + col as u16, y)) {
                    cell.set_char(WAVEFORM_CHARS[char_idx]).set_style(style);
                }
            }

            // Lower half (below center)
            for dy in 0..bar_height.min(max_half) {
                let y = center_y + dy + 1;
                if y < grid_y + grid_height {
                    let frac = (dy + 1) as f32 / max_half as f32;
                    let color = waveform_color(frac);
                    let style = ratatui::style::Style::from(Style::new().fg(color));
                    let char_idx = if dy + 1 == bar_height { ((amplitude * 7.0) as usize).min(7) } else { 7 };
                    if let Some(cell) = buf.cell_mut((grid_x + col as u16, y)) {
                        cell.set_char(WAVEFORM_CHARS[char_idx]).set_style(style);
                    }
                }
            }
        }

        // Status line
        let status_y = grid_y + grid_height;
        let status = format!("Samples: {}", waveform_len);
        Paragraph::new(Line::from(Span::styled(
            status,
            ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
        ))).render(RatatuiRect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
