use std::collections::VecDeque;

use super::{Color, Graphics, Rect, Style};
use crate::state::SessionState;

const CONSOLE_LINES: u16 = 4;
const CONSOLE_CAPACITY: usize = 100;

/// Block characters for vertical meter: ▁▂▃▄▅▆▇█ (U+2581–U+2588)
const BLOCK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Captured view state for back/forward navigation
#[derive(Debug, Clone)]
pub struct ViewState {
    pub pane_id: String,
    pub strip_selection: Option<usize>,
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

    /// Render vertical master meter on the right side of the frame
    fn render_master_meter(&self, g: &mut dyn Graphics, width: u16, _height: u16, sep_y: u16) {
        // Meter column: 2 chars from right border (border + 1 padding)
        let meter_x = width.saturating_sub(3);
        let meter_top = 2_u16; // below header
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

            if filled_sub >= row_end {
                g.set_style(Style::new().fg(color));
                g.put_char(meter_x, y, '█');
            } else if filled_sub > row_start {
                let sub_level = (filled_sub - row_start) as usize;
                g.set_style(Style::new().fg(color));
                g.put_char(meter_x, y, BLOCK_CHARS[sub_level.saturating_sub(1).min(7)]);
            } else {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_char(meter_x, y, '·');
            }
        }

        // Label below meter
        let label_y = meter_top + meter_height;
        if self.master_mute {
            g.set_style(Style::new().fg(Color::MUTE_COLOR).bold());
            g.put_char(meter_x, label_y, 'M');
        } else {
            // dB value
            let db = if level <= 0.0 {
                "-∞".to_string()
            } else {
                let db = 20.0 * level.log10();
                format!("{:+.0}", db.max(-99.0))
            };
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            // Right-align in the available space
            let db_x = meter_x.saturating_sub(db.len() as u16 - 1);
            g.put_str(db_x, label_y, &db);
        }
    }

    /// Calculate the inner rect where pane content should render
    #[allow(dead_code)]
    pub fn inner_rect(width: u16, height: u16) -> Rect {
        // Inset: 1 left, 1 right, 1 top (border+header), CONSOLE_LINES+2 bottom (separator+console+border)
        let inner_x = 1;
        let inner_y = 2;
        let inner_w = width.saturating_sub(2);
        let inner_h = height.saturating_sub(2 + CONSOLE_LINES + 2);
        Rect::new(inner_x, inner_y, inner_w, inner_h)
    }

    /// Render the frame (border, header, console) around pane content.
    /// Takes a reference to SessionState for displaying BPM, key, scale, etc.
    pub fn render(&self, g: &mut dyn Graphics, session: &SessionState) {
        let (width, height) = g.size();
        if width < 10 || height < 10 {
            return;
        }

        // Outer border
        let rect = Rect::new(0, 0, width, height);
        g.set_style(Style::new().fg(Color::GRAY));
        g.draw_box(rect, None);

        // Top bar
        let snap_text = if session.snap { "ON" } else { "OFF" };
        let tuning_str = format!("A{:.0}", session.tuning_a4);
        let header = format!(
            " TUIDAW - {}     Key: {}  Scale: {}  BPM: {}  {}/{}  Tuning: {}  [Snap: {}] ",
            self.project_name, session.key.name(), session.scale.name(), session.bpm,
            session.time_signature.0, session.time_signature.1,
            tuning_str, snap_text,
        );
        g.set_style(Style::new().fg(Color::CYAN).bold());
        g.put_str(1, 0, &header);

        // Fill remaining top border after header with horizontal line
        let header_end = 1 + header.len() as u16;
        g.set_style(Style::new().fg(Color::GRAY));
        for x in header_end..width.saturating_sub(1) {
            g.put_char(x, 0, '─');
        }

        // Console separator line
        let sep_y = height.saturating_sub(CONSOLE_LINES + 2);
        g.set_style(Style::new().fg(Color::GRAY));
        g.put_char(0, sep_y, '├');
        for x in 1..width.saturating_sub(1) {
            g.put_char(x, sep_y, '─');
        }
        g.put_char(width.saturating_sub(1), sep_y, '┤');

        // Master meter (right side, inside border)
        self.render_master_meter(g, width, height, sep_y);

        // Console messages (last CONSOLE_LINES messages)
        let console_y = sep_y + 1;
        let msg_count = self.messages.len();
        let skip = msg_count.saturating_sub(CONSOLE_LINES as usize);
        let max_width = (width - 4) as usize;

        for (i, msg) in self.messages.iter().skip(skip).enumerate() {
            if i >= CONSOLE_LINES as usize {
                break;
            }
            let y = console_y + i as u16;
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            g.put_str(2, y, "> ");
            g.set_style(Style::new().fg(Color::SKY_BLUE));
            let truncated: String = msg.chars().take(max_width).collect();
            g.put_str(4, y, &truncated);
        }
    }
}
