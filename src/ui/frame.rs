use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::{Color, Style};
use crate::state::SessionState;

const CONSOLE_LINES: u16 = 4;
const CONSOLE_CAPACITY: usize = 100;

/// Block characters for vertical meter: ▁▂▃▄▅▆▇█ (U+2581–U+2588)
const BLOCK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Captured view state for back/forward navigation
#[derive(Debug, Clone)]
pub struct ViewState {
    pub pane_id: String,
    pub inst_selection: Option<usize>,
    pub edit_tab: u8,
}

/// Frame wrapping the active pane with border, header bar, and message console
pub struct Frame {
    messages: VecDeque<String>,
    pub project_name: String,
    pub master_mute: bool,
    /// Raw peak from audio engine (0.0–1.0+)
    master_peak: f32,
    /// Smoothed display value (fast attack, slow decay)
    peak_display: f32,
    /// Previous view for back navigation (backtick)
    pub back_view: Option<ViewState>,
    /// Forward view for forward navigation (backslash)
    pub forward_view: Option<ViewState>,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::with_capacity(CONSOLE_CAPACITY),
            project_name: "default".to_string(),
            master_mute: false,
            master_peak: 0.0,
            peak_display: 0.0,
            back_view: None,
            forward_view: None,
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

    /// Push a message to the console ring buffer
    pub fn push_message(&mut self, msg: String) {
        if self.messages.len() >= CONSOLE_CAPACITY {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
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
    pub fn render_buf(&self, area: RatatuiRect, buf: &mut Buffer, session: &SessionState) {
        if area.width < 10 || area.height < 10 {
            return;
        }

        let border_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));

        // Outer border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        block.render(area, buf);

        // Header line in the top border
        let snap_text = if session.snap { "ON" } else { "OFF" };
        let tuning_str = format!("A{:.0}", session.tuning_a4);
        let header = format!(
            " ILEX - {}     Key: {}  Scale: {}  BPM: {}  {}/{}  Tuning: {}  [Snap: {}] ",
            self.project_name, session.key.name(), session.scale.name(), session.bpm,
            session.time_signature.0, session.time_signature.1,
            tuning_str, snap_text,
        );
        let header_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold());
        Paragraph::new(Line::from(Span::styled(&header, header_style)))
            .render(RatatuiRect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1), buf);

        // Fill remaining top border after header
        let header_end = area.x + 1 + header.len() as u16;
        for x in header_end..area.x + area.width.saturating_sub(1) {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char('─').set_style(border_style);
            }
        }

        // Console separator line
        let sep_y = area.y + area.height.saturating_sub(CONSOLE_LINES + 2);
        if let Some(cell) = buf.cell_mut((area.x, sep_y)) {
            cell.set_char('├').set_style(border_style);
        }
        for x in (area.x + 1)..(area.x + area.width.saturating_sub(1)) {
            if let Some(cell) = buf.cell_mut((x, sep_y)) {
                cell.set_char('─').set_style(border_style);
            }
        }
        if let Some(cell) = buf.cell_mut((area.x + area.width.saturating_sub(1), sep_y)) {
            cell.set_char('┤').set_style(border_style);
        }

        // Master meter (direct buffer writes)
        self.render_master_meter_buf(buf, area.width, area.height, sep_y);

        // Console messages
        let console_y = sep_y + 1;
        let msg_count = self.messages.len();
        let skip = msg_count.saturating_sub(CONSOLE_LINES as usize);
        let max_width = (area.width - 4) as usize;

        let prompt_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let msg_style = ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE));

        for (i, msg) in self.messages.iter().skip(skip).enumerate() {
            if i >= CONSOLE_LINES as usize {
                break;
            }
            let y = console_y + i as u16;
            let truncated: String = msg.chars().take(max_width).collect();
            let line = Line::from(vec![
                Span::styled("> ", prompt_style),
                Span::styled(truncated, msg_style),
            ]);
            Paragraph::new(line).render(
                RatatuiRect::new(area.x + 2, y, area.width.saturating_sub(4), 1), buf,
            );
        }
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
