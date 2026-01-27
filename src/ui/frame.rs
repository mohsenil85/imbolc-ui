use std::collections::VecDeque;

use super::{Color, Graphics, Rect, Style};
use crate::state::music::{Key, Scale};

const CONSOLE_LINES: u16 = 4;
const CONSOLE_CAPACITY: usize = 100;

/// Musical session state displayed in the frame header
pub struct SessionState {
    pub key: Key,
    pub scale: Scale,
    pub bpm: u16,
    pub tuning_a4: f32,
    pub snap: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            key: Key::C,
            scale: Scale::Major,
            bpm: 120,
            tuning_a4: 440.0,
            snap: false,
        }
    }
}

/// Frame wrapping the active pane with border, header bar, and message console
pub struct Frame {
    messages: VecDeque<String>,
    pub session: SessionState,
    pub project_name: String,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::with_capacity(CONSOLE_CAPACITY),
            session: SessionState::default(),
            project_name: "default".to_string(),
        }
    }

    pub fn set_project_name(&mut self, name: String) {
        self.project_name = name;
    }

    /// Push a message to the console ring buffer
    pub fn push_message(&mut self, msg: String) {
        if self.messages.len() >= CONSOLE_CAPACITY {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    /// Calculate the inner rect where pane content should render
    pub fn inner_rect(width: u16, height: u16) -> Rect {
        // Inset: 1 left, 1 right, 1 top (border+header), CONSOLE_LINES+2 bottom (separator+console+border)
        let inner_x = 1;
        let inner_y = 2;
        let inner_w = width.saturating_sub(2);
        let inner_h = height.saturating_sub(2 + CONSOLE_LINES + 2);
        Rect::new(inner_x, inner_y, inner_w, inner_h)
    }

    /// Render the frame (border, header, console) around pane content
    pub fn render(&self, g: &mut dyn Graphics) {
        let (width, height) = g.size();
        if width < 10 || height < 10 {
            return;
        }

        // Outer border
        let rect = Rect::new(0, 0, width, height);
        g.set_style(Style::new().fg(Color::GRAY));
        g.draw_box(rect, None);

        // Top bar
        let snap_text = if self.session.snap { "ON" } else { "OFF" };
        let tuning_str = format!("A{:.0}", self.session.tuning_a4);
        let header = format!(
            " TUIDAW - {}     Key: {}  Scale: {}  BPM: {}  Tuning: {}  [Snap: {}] ",
            self.project_name, self.session.key.name(), self.session.scale.name(), self.session.bpm,
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
