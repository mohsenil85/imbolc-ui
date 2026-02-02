use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::{Color, Style};
use crate::state::AppState;

/// Block characters for vertical meter: ▁▂▃▄▅▆▇█ (U+2581–U+2588)
const BLOCK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Captured view state for back/forward navigation
#[derive(Debug, Clone)]
pub struct ViewState {
    pub pane_id: String,
    pub inst_selection: Option<usize>,
    pub edit_tab: u8,
}

/// Frame wrapping the active pane with border and header bar
pub struct Frame {
    pub project_name: String,
    pub master_mute: bool,
    /// Raw peak from audio engine (0.0–1.0+)
    master_peak: f32,
    /// Smoothed display value (fast attack, slow decay)
    peak_display: f32,
    /// Navigation history (browser-style)
    pub view_history: Vec<ViewState>,
    /// Current position in view_history
    pub history_cursor: usize,
    /// Whether audio is currently being recorded
    pub recording: bool,
    /// Elapsed recording time in seconds
    pub recording_secs: u64,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            project_name: "default".to_string(),
            master_mute: false,
            master_peak: 0.0,
            peak_display: 0.0,
            view_history: Vec::new(),
            history_cursor: 0,
            recording: false,
            recording_secs: 0,
        }
    }

    pub fn set_project_name(&mut self, name: String) {
        self.project_name = name;
    }

    /// Update master meter from real audio peak (call each frame from main loop)
    pub fn set_master_peak(&mut self, peak: f32, mute: bool) {
        self.master_peak = peak;
        self.master_mute = mute;
        // Fast attack, slow decay
        self.peak_display = peak.max(self.peak_display * 0.85);
    }

    /// Get meter color for a given row position (0=bottom, height-1=top)
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

    /// Render the frame using ratatui buffer directly.
    pub fn render_buf(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        if area.width < 10 || area.height < 10 {
            return;
        }

        let session = &state.session;
        let border_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));

        // Outer border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        block.render(area, buf);

        // Header line in the top border (left-aligned)
        let snap_text = if session.snap { "ON" } else { "OFF" };
        let tuning_str = format!("A{:.0}", session.tuning_a4);
        let dirty_indicator = if state.dirty { "*" } else { "" };
        let header = format!(
            " IMBOLC - {}{}  Key: {}  Scale: {}  BPM: {}  {}/{}  Tuning: {}  [Snap: {}] ",
            self.project_name, dirty_indicator,
            session.key.name(), session.scale.name(), session.bpm,
            session.time_signature.0, session.time_signature.1,
            tuning_str, snap_text,
        );
        let header_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold());
        Paragraph::new(Line::from(Span::styled(&header, header_style)))
            .render(RatatuiRect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1), buf);

        // Right-aligned items: [instrument indicator] [REC indicator]
        let inst_indicator = if let Some(idx) = state.instruments.selected {
            if let Some(inst) = state.instruments.instruments.get(idx) {
                format!(" {}: {} ", idx + 1, inst.name)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let rec_text = if self.recording {
            let mins = self.recording_secs / 60;
            let secs = self.recording_secs % 60;
            format!(" REC {:02}:{:02} ", mins, secs)
        } else {
            String::new()
        };

        // Position right-aligned items from the right edge inward
        let right_edge = area.x + area.width.saturating_sub(1);
        let mut cursor = right_edge;

        // REC indicator (rightmost)
        if self.recording {
            let rec_start = cursor.saturating_sub(rec_text.len() as u16);
            let rec_style = ratatui::style::Style::from(Style::new().fg(Color::MUTE_COLOR).bold());
            for (j, ch) in rec_text.chars().enumerate() {
                let rx = rec_start + j as u16;
                if rx < right_edge {
                    if let Some(cell) = buf.cell_mut((rx, area.y)) {
                        cell.set_char(ch).set_style(rec_style);
                    }
                }
            }
            cursor = rec_start;
        }

        // Instrument indicator (to the left of REC)
        if !inst_indicator.is_empty() {
            let inst_start = cursor.saturating_sub(inst_indicator.len() as u16);
            let inst_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold());
            for (j, ch) in inst_indicator.chars().enumerate() {
                let ix = inst_start + j as u16;
                if ix < cursor {
                    if let Some(cell) = buf.cell_mut((ix, area.y)) {
                        cell.set_char(ch).set_style(inst_style);
                    }
                }
            }
            cursor = inst_start;
        }

        // Fill gap between header and right-aligned items with border
        let header_end = area.x + 1 + header.len() as u16;
        for x in header_end..cursor {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char('─').set_style(border_style);
            }
        }

        // Master meter (direct buffer writes)
        let meter_bottom_y = area.y + area.height.saturating_sub(2);
        self.render_master_meter_buf(buf, area.width, area.height, meter_bottom_y);
    }

    /// Render vertical master meter on the right side (buffer version)
    fn render_master_meter_buf(&self, buf: &mut Buffer, width: u16, _height: u16, sep_y: u16) {
        let meter_x = width.saturating_sub(3);
        let meter_top = 2_u16;
        let meter_height = sep_y.saturating_sub(meter_top + 1);

        if meter_height < 3 {
            return;
        }

        let level = if self.master_mute { 0.0 } else { self.peak_display.min(1.0) };
        let total_sub = meter_height as f32 * 8.0;
        let filled_sub = (level * total_sub) as u16;

        for row in 0..meter_height {
            let inverted_row = meter_height - 1 - row;
            let y = meter_top + row;
            let row_start = inverted_row * 8;
            let row_end = row_start + 8;
            let color = Self::meter_color(inverted_row, meter_height);

            let (ch, c) = if filled_sub >= row_end {
                ('█', color)
            } else if filled_sub > row_start {
                let sub_level = (filled_sub - row_start) as usize;
                (BLOCK_CHARS[sub_level.saturating_sub(1).min(7)], color)
            } else {
                ('·', Color::DARK_GRAY)
            };

            if let Some(cell) = buf.cell_mut((meter_x, y)) {
                cell.set_char(ch).set_style(ratatui::style::Style::from(Style::new().fg(c)));
            }
        }

        // Label below meter
        let label_y = meter_top + meter_height;
        if self.master_mute {
            if let Some(cell) = buf.cell_mut((meter_x, label_y)) {
                cell.set_char('M').set_style(
                    ratatui::style::Style::from(Style::new().fg(Color::MUTE_COLOR).bold()),
                );
            }
        } else {
            let db = if level <= 0.0 {
                "-∞".to_string()
            } else {
                let db_val = 20.0 * level.log10();
                format!("{:+.0}", db_val.max(-99.0))
            };
            let db_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
            let db_x = meter_x.saturating_sub(db.len() as u16 - 1);
            for (j, ch) in db.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((db_x + j as u16, label_y)) {
                    cell.set_char(ch).set_style(db_style);
                }
            }
        }
    }

}
