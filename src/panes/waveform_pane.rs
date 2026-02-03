use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Rect, RenderBuf, Action, Color, InputEvent, Keymap, Pane, Style};

/// Waveform display characters (8 levels)
const WAVEFORM_CHARS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

/// Spectrum band labels
const SPECTRUM_LABELS: [&str; 7] = ["60", "150", "400", "1k", "2.5k", "6k", "15k"];

/// Color a waveform/meter row by its distance from center (0.0=center, 1.0=edge)
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

/// Convert linear amplitude to dB
fn amp_to_db(amp: f32) -> f32 {
    if amp <= 0.0 { -96.0 } else { 20.0 * amp.log10() }
}

/// Display mode for the waveform pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaveformMode {
    Waveform,
    Spectrum,
    Oscilloscope,
    LufsMeter,
}

#[allow(dead_code)]
impl WaveformMode {
    fn next(self) -> Self {
        match self {
            WaveformMode::Waveform => WaveformMode::Spectrum,
            WaveformMode::Spectrum => WaveformMode::Oscilloscope,
            WaveformMode::Oscilloscope => WaveformMode::LufsMeter,
            WaveformMode::LufsMeter => WaveformMode::Waveform,
        }
    }

    fn name(self) -> &'static str {
        match self {
            WaveformMode::Waveform => "Waveform",
            WaveformMode::Spectrum => "Spectrum",
            WaveformMode::Oscilloscope => "Oscilloscope",
            WaveformMode::LufsMeter => "Level Meter",
        }
    }
}

pub struct WaveformPane {
    keymap: Keymap,
    /// Live waveform from audio input
    pub audio_in_waveform: Option<Vec<f32>>,
    /// Current display mode
    mode: WaveformMode,
}

impl WaveformPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            audio_in_waveform: None,
            mode: WaveformMode::Waveform,
        }
    }
}

impl Default for WaveformPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl WaveformPane {
    fn render_waveform(&self, area: Rect, buf: &mut Buffer, state: &AppState) {
        let is_recorded = state.recorded_waveform_peaks.is_some();
        let waveform = state.recorded_waveform_peaks.as_deref()
            .or(self.audio_in_waveform.as_deref())
            .unwrap_or(&[]);

        let rect = center_rect(area, 97, 29);
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + 1;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(2);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        let title = if is_recorded {
            " Recorded Waveform ".to_string()
        } else if let Some(inst) = state.instruments.selected_instrument() {
            format!(" Audio Input: {} ", inst.name)
        } else {
            " Audio Input ".to_string()
        };
        self.render_border(rect, buf, &title, Color::AUDIO_IN_COLOR);
        self.render_header(rect, buf, state, "Waveform");

        // Center line
        let center_y = grid_y + grid_height / 2;
        let half_height = (grid_height / 2) as f32;
        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        for x in 0..grid_width {
            if let Some(cell) = buf.cell_mut((grid_x + x, center_y)) {
                cell.set_char('\u{2500}').set_style(dark_gray);
            }
        }

        // Draw waveform
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

        let status_y = grid_y + grid_height;
        let status = format!("Samples: {}  [Tab: cycle mode]", waveform_len);
        Paragraph::new(Line::from(Span::styled(
            status,
            ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
        ))).render(Rect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);
    }

    fn render_spectrum(&self, area: Rect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);
        let header_height: u16 = 2;
        let footer_height: u16 = 3;
        let grid_x = rect.x + 1;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(2);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        self.render_border(rect, buf, " Spectrum Analyzer ", Color::METER_LOW);
        self.render_header(rect, buf, state, "Spectrum");

        let bands = &state.visualization.spectrum_bands;
        let num_bands = bands.len();
        let band_width = grid_width as usize / num_bands;
        let gap = 1_usize; // gap between bands

        for (i, &amp) in bands.iter().enumerate() {
            let bar_x = grid_x + (i * band_width) as u16 + 1;
            let bar_width = (band_width - gap).max(1);
            let bar_height = (amp.min(1.0) * grid_height as f32) as u16;

            // Draw bar from bottom up
            for dy in 0..bar_height.min(grid_height) {
                let y = grid_y + grid_height - 1 - dy;
                let frac = (dy + 1) as f32 / grid_height as f32;
                let color = waveform_color(frac);
                let style = ratatui::style::Style::from(Style::new().fg(color));
                for bx in 0..bar_width as u16 {
                    if bar_x + bx < grid_x + grid_width {
                        if let Some(cell) = buf.cell_mut((bar_x + bx, y)) {
                            cell.set_char(WAVEFORM_CHARS[7]).set_style(style);
                        }
                    }
                }
            }

            // Label below
            let label_y = grid_y + grid_height;
            let label = SPECTRUM_LABELS[i];
            let label_x = bar_x + (bar_width as u16 / 2).saturating_sub(label.len() as u16 / 2);
            Paragraph::new(Line::from(Span::styled(
                label,
                ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
            ))).render(Rect::new(label_x, label_y, label.len() as u16 + 1, 1), buf);

            // dB value above
            let db = amp_to_db(amp);
            let db_str = if db <= -60.0 { "-inf".to_string() } else { format!("{:.0}", db) };
            let db_x = bar_x + (bar_width as u16 / 2).saturating_sub(db_str.len() as u16 / 2);
            let db_y = grid_y + grid_height + 1;
            if db_y < rect.y + rect.height - 1 {
                Paragraph::new(Line::from(Span::styled(
                    db_str,
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                ))).render(Rect::new(db_x, db_y, 5, 1), buf);
            }
        }

        let status_y = rect.y + rect.height - 2;
        Paragraph::new(Line::from(Span::styled(
            "[Tab: cycle mode]",
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(Rect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);
    }

    fn render_oscilloscope(&self, area: Rect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + 1;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(2);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        self.render_border(rect, buf, " Oscilloscope ", Color::MIDI_COLOR);
        self.render_header(rect, buf, state, "Oscilloscope");

        let scope = &state.visualization.scope_buffer;
        let center_y = grid_y + grid_height / 2;
        let half_height = (grid_height / 2) as f32;

        // Draw center line
        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        for x in 0..grid_width {
            if let Some(cell) = buf.cell_mut((grid_x + x, center_y)) {
                cell.set_char('\u{2500}').set_style(dark_gray);
            }
        }

        // Draw scope trace
        let scope_len = scope.len();
        let green = ratatui::style::Style::from(Style::new().fg(Color::new(60, 200, 80)));
        for col in 0..grid_width as usize {
            let sample_idx = if scope_len > 0 {
                (col * scope_len / grid_width as usize).min(scope_len - 1)
            } else {
                continue;
            };
            let sample = scope[sample_idx].clamp(-1.0, 1.0);
            let pixel_y = center_y as f32 - (sample * half_height);
            let y = (pixel_y as u16).clamp(grid_y, grid_y + grid_height - 1);
            if let Some(cell) = buf.cell_mut((grid_x + col as u16, y)) {
                cell.set_char('\u{2588}').set_style(green);
            }

            // Draw a connecting line between consecutive samples
            if col > 0 && scope_len > 1 {
                let prev_idx = ((col - 1) * scope_len / grid_width as usize).min(scope_len - 1);
                let prev_sample = scope[prev_idx].clamp(-1.0, 1.0);
                let prev_pixel_y = center_y as f32 - (prev_sample * half_height);
                let prev_y = (prev_pixel_y as u16).clamp(grid_y, grid_y + grid_height - 1);
                let (y_min, y_max) = if y < prev_y { (y, prev_y) } else { (prev_y, y) };
                for fill_y in y_min..=y_max {
                    if fill_y >= grid_y && fill_y < grid_y + grid_height {
                        if let Some(cell) = buf.cell_mut((grid_x + col as u16, fill_y)) {
                            cell.set_char('\u{2588}').set_style(green);
                        }
                    }
                }
            }
        }

        // +1/-1 labels
        let plus_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        Paragraph::new(Line::from(Span::styled("+1", plus_style)))
            .render(Rect::new(grid_x, grid_y, 2, 1), buf);
        Paragraph::new(Line::from(Span::styled("-1", plus_style)))
            .render(Rect::new(grid_x, grid_y + grid_height - 1, 2, 1), buf);

        let status_y = grid_y + grid_height;
        let status = format!("Samples: {}  [Tab: cycle mode]", scope_len);
        Paragraph::new(Line::from(Span::styled(
            status,
            ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
        ))).render(Rect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);
    }

    fn render_lufs_meter(&self, area: Rect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);
        let header_height: u16 = 2;
        let footer_height: u16 = 2;
        let grid_x = rect.x + 1;
        let grid_y = rect.y + header_height;
        let grid_width = rect.width.saturating_sub(2);
        let grid_height = rect.height.saturating_sub(header_height + footer_height + 1);

        self.render_border(rect, buf, " Level Meter ", Color::METER_LOW);
        self.render_header(rect, buf, state, "Level Meter");

        let viz = &state.visualization;
        let meter_width = grid_width / 2 - 4; // space for each channel

        // Left channel
        self.render_single_meter(grid_x + 2, grid_y, meter_width, grid_height, viz.peak_l, viz.rms_l, "L", buf);

        // Right channel
        self.render_single_meter(grid_x + grid_width / 2 + 2, grid_y, meter_width, grid_height, viz.peak_r, viz.rms_r, "R", buf);

        // Numeric readout at bottom
        let status_y = grid_y + grid_height;
        let peak_db_l = amp_to_db(viz.peak_l);
        let peak_db_r = amp_to_db(viz.peak_r);
        let rms_db_l = amp_to_db(viz.rms_l);
        let rms_db_r = amp_to_db(viz.rms_r);
        let status = format!(
            "L: peak {:.1}dB  rms {:.1}dB    R: peak {:.1}dB  rms {:.1}dB    [Tab: cycle mode]",
            peak_db_l, rms_db_l, peak_db_r, rms_db_r,
        );
        Paragraph::new(Line::from(Span::styled(
            status,
            ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
        ))).render(Rect::new(rect.x + 1, status_y, rect.width.saturating_sub(2), 1), buf);
    }

    fn render_single_meter(&self, x: u16, y: u16, width: u16, height: u16, peak: f32, rms: f32, label: &str, buf: &mut Buffer) {
        // dB scale: -60 to 0
        let db_range = 60.0_f32;
        let peak_db = amp_to_db(peak).max(-db_range);
        let rms_db = amp_to_db(rms).max(-db_range);
        let peak_frac = ((peak_db + db_range) / db_range).clamp(0.0, 1.0);
        let rms_frac = ((rms_db + db_range) / db_range).clamp(0.0, 1.0);

        let peak_height = (peak_frac * height as f32) as u16;
        let rms_height = (rms_frac * height as f32) as u16;

        // Split width: RMS bars take most of it, peak indicator on the side
        let rms_width = width.saturating_sub(2);

        // Draw RMS bars from bottom up
        for dy in 0..rms_height.min(height) {
            let row = y + height - 1 - dy;
            let frac = (dy + 1) as f32 / height as f32;
            let color = waveform_color(frac);
            let style = ratatui::style::Style::from(Style::new().fg(color));
            for bx in 0..rms_width {
                if let Some(cell) = buf.cell_mut((x + bx, row)) {
                    cell.set_char(WAVEFORM_CHARS[7]).set_style(style);
                }
            }
        }

        // Draw peak indicator (single character on the right side)
        if peak_height > 0 {
            let peak_y = y + height - peak_height.min(height);
            let peak_frac_color = peak_height as f32 / height as f32;
            let peak_color = waveform_color(peak_frac_color);
            let peak_style = ratatui::style::Style::from(Style::new().fg(peak_color));
            if let Some(cell) = buf.cell_mut((x + rms_width + 1, peak_y)) {
                cell.set_char('\u{2501}').set_style(peak_style);
            }
        }

        // Channel label
        let label_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
        let label_x = x + rms_width / 2;
        let label_y = y + height;
        if label_y < y + height + 2 {
            Paragraph::new(Line::from(Span::styled(label, label_style)))
                .render(Rect::new(label_x, label_y, 2, 1), buf);
        }

        // dB scale markers on the left side of meter
        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let markers = [("0", 0.0), ("-6", 6.0), ("-12", 12.0), ("-24", 24.0), ("-48", 48.0)];
        for (text, db_offset) in markers {
            let frac = (db_range - db_offset) / db_range;
            let marker_y = y + ((1.0 - frac) * height as f32) as u16;
            if marker_y >= y && marker_y < y + height {
                // Tick mark
                if x > 0 {
                    Paragraph::new(Line::from(Span::styled(text, dark_gray)))
                        .render(Rect::new(x.saturating_sub(text.len() as u16 + 1), marker_y, text.len() as u16, 1), buf);
                }
            }
        }
    }

    fn render_border(&self, rect: Rect, buf: &mut Buffer, title: &str, color: Color) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(ratatui::style::Style::from(Style::new().fg(color)))
            .title_style(ratatui::style::Style::from(Style::new().fg(color)));
        block.render(rect, buf);
    }

    fn render_header(&self, rect: Rect, buf: &mut Buffer, state: &AppState, mode_name: &str) {
        let piano_roll = &state.session.piano_roll;
        let header_y = rect.y + 1;
        let play_icon = if piano_roll.playing { "||" } else { "> " };
        let header_text = format!(
            " BPM:{:.0}  {}  {}",
            state.audio_bpm, play_icon, mode_name,
        );
        Paragraph::new(Line::from(Span::styled(
            header_text,
            ratatui::style::Style::from(Style::new().fg(Color::WHITE)),
        ))).render(Rect::new(rect.x + 1, header_y, rect.width.saturating_sub(2), 1), buf);
    }
}

impl Pane for WaveformPane {
    fn id(&self) -> &'static str {
        "waveform"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "cycle_mode" => {
                self.mode = self.mode.next();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut RenderBuf, state: &AppState) {
        let buf = buf.raw_buf();
        match self.mode {
            WaveformMode::Waveform => self.render_waveform(area, buf, state),
            WaveformMode::Spectrum => self.render_spectrum(area, buf, state),
            WaveformMode::Oscilloscope => self.render_oscilloscope(area, buf, state),
            WaveformMode::LufsMeter => self.render_lufs_meter(area, buf, state),
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
