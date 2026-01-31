use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::drum_sequencer::NUM_PADS;
use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, MouseEvent, MouseEventKind, MouseButton, NavAction, Pane, SequencerAction, Style};

pub struct SequencerPane {
    keymap: Keymap,
    cursor_pad: usize,
    cursor_step: usize,
    view_start_step: usize,
}

impl SequencerPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            cursor_pad: 0,
            cursor_step: 0,
            view_start_step: 0,
        }
    }

    fn visible_steps(&self, box_width: u16) -> usize {
        // Pad label column: 11 chars, box borders: 4 chars, step columns: 3 chars each
        let available = (box_width as usize).saturating_sub(15);
        available / 3
    }

}

impl Default for SequencerPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for SequencerPane {
    fn id(&self) -> &'static str {
        "sequencer"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        let seq = match state.instruments.selected_drum_sequencer() {
            Some(s) => s,
            None => return Action::None,
        };
        let pattern_length = seq.pattern().length;

        match action {
            "vel_up" => {
                return Action::Sequencer(SequencerAction::AdjustVelocity(
                    self.cursor_pad,
                    self.cursor_step,
                    10,
                ));
            }
            "vel_down" => {
                return Action::Sequencer(SequencerAction::AdjustVelocity(
                    self.cursor_pad,
                    self.cursor_step,
                    -10,
                ));
            }
            "pad_level_down" => {
                return Action::Sequencer(SequencerAction::AdjustPadLevel(
                    self.cursor_pad,
                    -0.05,
                ));
            }
            "pad_level_up" => {
                return Action::Sequencer(SequencerAction::AdjustPadLevel(
                    self.cursor_pad,
                    0.05,
                ));
            }
            "up" => {
                self.cursor_pad = self.cursor_pad.saturating_sub(1);
                Action::None
            }
            "down" => {
                self.cursor_pad = (self.cursor_pad + 1).min(NUM_PADS - 1);
                Action::None
            }
            "left" => {
                self.cursor_step = self.cursor_step.saturating_sub(1);
                Action::None
            }
            "right" => {
                self.cursor_step = (self.cursor_step + 1).min(pattern_length - 1);
                Action::None
            }
            "toggle" => Action::Sequencer(SequencerAction::ToggleStep(
                self.cursor_pad,
                self.cursor_step,
            )),
            "play_stop" => Action::Sequencer(SequencerAction::PlayStop),
            "load_sample" => {
                Action::Sequencer(SequencerAction::LoadSample(self.cursor_pad))
            }
            "chopper" => Action::Nav(NavAction::PushPane("sample_chopper")),
            "clear_pad" => Action::Sequencer(SequencerAction::ClearPad(self.cursor_pad)),
            "clear_pattern" => Action::Sequencer(SequencerAction::ClearPattern),
            "prev_pattern" => Action::Sequencer(SequencerAction::PrevPattern),
            "next_pattern" => Action::Sequencer(SequencerAction::NextPattern),
            "cycle_length" => Action::Sequencer(SequencerAction::CyclePatternLength),
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let box_width: u16 = 97;
        let rect = center_rect(area, box_width, 29);

        let seq = match state.instruments.selected_drum_sequencer() {
            Some(s) => s,
            None => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(" Drum Sequencer ")
                    .border_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)))
                    .title_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)));
                block.render(rect, buf);
                let cy = rect.y + rect.height / 2;
                Paragraph::new(Line::from(Span::styled(
                    "No drum machine instrument selected. Press 1 to add one.",
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                ))).render(RatatuiRect::new(rect.x + 12, cy, rect.width.saturating_sub(14), 1), buf);
                return;
            }
        };
        let pattern = seq.pattern();
        let visible = self.visible_steps(box_width);

        // Calculate effective scroll
        let mut view_start = self.view_start_step;
        if self.cursor_step < view_start {
            view_start = self.cursor_step;
        } else if self.cursor_step >= view_start + visible {
            view_start = self.cursor_step - visible + 1;
        }
        if view_start + visible > pattern.length {
            view_start = pattern.length.saturating_sub(visible);
        }

        let steps_shown = visible.min(pattern.length - view_start);

        // Draw box
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Drum Sequencer ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::ORANGE)));
        block.render(rect, buf);

        let cx = rect.x + 2;
        let cy = rect.y + 1;

        // Header line
        let pattern_label = match seq.current_pattern {
            0 => "A", 1 => "B", 2 => "C", 3 => "D", _ => "?",
        };
        let play_label = if seq.playing { "PLAY" } else { "STOP" };
        let play_color = if seq.playing { Color::GREEN } else { Color::GRAY };

        let header = Line::from(vec![
            Span::styled(
                format!("Pattern {}", pattern_label),
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold()),
            ),
            Span::styled(
                format!("  Length: {}", pattern.length),
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ),
            Span::styled(
                format!("  BPM: {:.0}", state.session.piano_roll.bpm),
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ),
            Span::styled(
                format!("  {}", play_label),
                ratatui::style::Style::from(Style::new().fg(play_color).bold()),
            ),
        ]);
        Paragraph::new(header).render(RatatuiRect::new(cx, cy, rect.width.saturating_sub(4), 1), buf);

        // Step number header
        let header_y = cy + 2;
        let label_width: u16 = 11;
        let step_col_start = cx + label_width;

        let dark_gray = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        for i in 0..steps_shown {
            let step_num = view_start + i + 1;
            let x = step_col_start + (i as u16) * 3;
            let num_str = if step_num < 10 {
                format!(" {}", step_num)
            } else {
                format!("{:2}", step_num)
            };
            for (j, ch) in num_str.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((x + j as u16, header_y)) {
                    cell.set_char(ch).set_style(dark_gray);
                }
            }
        }

        // Grid rows
        let grid_y = header_y + 1;

        for pad_idx in 0..NUM_PADS {
            let y = grid_y + pad_idx as u16;
            let is_cursor_row = pad_idx == self.cursor_pad;

            // Pad label
            let pad = &seq.pads[pad_idx];
            let label = if pad.name.is_empty() {
                format!("{:>2} ----   ", pad_idx + 1)
            } else {
                let name = if pad.name.len() > 6 { &pad.name[..6] } else { &pad.name };
                format!("{:>2} {:<6} ", pad_idx + 1, name)
            };

            let label_style = if is_cursor_row {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold())
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::GRAY))
            };
            for (j, ch) in label.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((cx + j as u16, y)) {
                    cell.set_char(ch).set_style(label_style);
                }
            }

            // Steps
            for i in 0..steps_shown {
                let step_idx = view_start + i;
                let x = step_col_start + (i as u16) * 3;
                let is_cursor = is_cursor_row && step_idx == self.cursor_step;
                let is_playhead = seq.playing && step_idx == seq.current_step;

                let step = &pattern.steps[pad_idx][step_idx];
                let is_beat = step_idx % 4 == 0;

                let (fg, bg) = if is_cursor {
                    if step.active { (Color::BLACK, Color::WHITE) } else { (Color::WHITE, Color::SELECTION_BG) }
                } else if is_playhead {
                    if step.active { (Color::BLACK, Color::GREEN) } else { (Color::GREEN, Color::new(20, 50, 20)) }
                } else if step.active {
                    let intensity = (step.velocity as f32 / 127.0 * 200.0) as u8 + 55;
                    (Color::new(intensity, intensity / 3, 0), Color::BLACK)
                } else if is_beat {
                    (Color::new(60, 60, 60), Color::BLACK)
                } else {
                    (Color::new(40, 40, 40), Color::BLACK)
                };

                let style = ratatui::style::Style::from(Style::new().fg(fg).bg(bg));
                let chars: Vec<char> = if step.active { " █ " } else { " · " }.chars().collect();
                for (j, ch) in chars.iter().enumerate() {
                    if let Some(cell) = buf.cell_mut((x + j as u16, y)) {
                        cell.set_char(*ch).set_style(style);
                    }
                }
            }
        }

        // Pad detail line
        let detail_y = grid_y + NUM_PADS as u16 + 1;
        let pad = &seq.pads[self.cursor_pad];

        let pad_label = format!("Pad {:>2}", self.cursor_pad + 1);
        Paragraph::new(Line::from(Span::styled(
            pad_label,
            ratatui::style::Style::from(Style::new().fg(Color::ORANGE).bold()),
        ))).render(RatatuiRect::new(cx, detail_y, 8, 1), buf);

        let name_display = if pad.name.is_empty() {
            "(no sample)"
        } else if pad.name.len() > 20 {
            &pad.name[..20]
        } else {
            &pad.name
        };
        Paragraph::new(Line::from(Span::styled(
            name_display,
            ratatui::style::Style::from(Style::new().fg(Color::WHITE)),
        ))).render(RatatuiRect::new(cx + 8, detail_y, 22, 1), buf);

        // Level bar
        let level_x = cx + 32;
        for (j, ch) in "Level:".chars().enumerate() {
            if let Some(cell) = buf.cell_mut((level_x + j as u16, detail_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }

        let bar_x = level_x + 7;
        let bar_width: usize = 10;
        let filled = (pad.level * bar_width as f32) as usize;
        for i in 0..bar_width {
            let (ch, style) = if i < filled {
                ('\u{2588}', ratatui::style::Style::from(Style::new().fg(Color::ORANGE)))
            } else {
                ('\u{2591}', ratatui::style::Style::from(Style::new().fg(Color::new(40, 40, 40))))
            };
            if let Some(cell) = buf.cell_mut((bar_x + i as u16, detail_y)) {
                cell.set_char(ch).set_style(style);
            }
        }

        // Velocity
        let step = &pattern.steps[self.cursor_pad][self.cursor_step];
        let vel_str = format!("Vel: {}", step.velocity);
        for (j, ch) in vel_str.chars().enumerate() {
            if let Some(cell) = buf.cell_mut((bar_x + bar_width as u16 + 2 + j as u16, detail_y)) {
                cell.set_char(ch).set_style(dark_gray);
            }
        }

        // Scroll indicator
        if pattern.length > visible {
            let scroll_str = format!("{}-{}/{}", view_start + 1, view_start + steps_shown, pattern.length);
            let scroll_x = rect.x + rect.width - 2 - scroll_str.len() as u16;
            for (j, ch) in scroll_str.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((scroll_x + j as u16, detail_y)) {
                    cell.set_char(ch).set_style(dark_gray);
                }
            }
        }

        // Help line
        let help_y = rect.y + rect.height - 2;
        Paragraph::new(Line::from(Span::styled(
            "Enter:toggle  Space:play/stop  s:sample  c:chopper  x:clear  []:pattern  {:length",
            ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
        ))).render(RatatuiRect::new(cx, help_y, rect.width.saturating_sub(4), 1), buf);
    }

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action {
        let box_width: u16 = 97;
        let rect = center_rect(area, box_width, 29);
        let cx = rect.x + 2;
        let header_y = rect.y + 3;
        let label_width: u16 = 11;
        let step_col_start = cx + label_width;
        let grid_y = header_y + 1;
        let visible = self.visible_steps(box_width);

        let seq = match state.instruments.selected_drum_sequencer() {
            Some(s) => s,
            None => return Action::None,
        };
        let pattern = seq.pattern();

        // Calculate effective scroll (same as render)
        let mut view_start = self.view_start_step;
        if self.cursor_step < view_start {
            view_start = self.cursor_step;
        } else if self.cursor_step >= view_start + visible {
            view_start = self.cursor_step - visible + 1;
        }
        if view_start + visible > pattern.length {
            view_start = pattern.length.saturating_sub(visible);
        }

        let col = event.column;
        let row = event.row;

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Click on step grid
                if col >= step_col_start && row >= grid_y && row < grid_y + NUM_PADS as u16 {
                    let pad_idx = (row - grid_y) as usize;
                    let step_offset = (col - step_col_start) / 3;
                    let step_idx = view_start + step_offset as usize;
                    if pad_idx < NUM_PADS && step_idx < pattern.length {
                        self.cursor_pad = pad_idx;
                        self.cursor_step = step_idx;
                        return Action::Sequencer(SequencerAction::ToggleStep(pad_idx, step_idx));
                    }
                }
                // Click on pad label to select pad
                if col >= cx && col < step_col_start && row >= grid_y && row < grid_y + NUM_PADS as u16 {
                    let pad_idx = (row - grid_y) as usize;
                    if pad_idx < NUM_PADS {
                        self.cursor_pad = pad_idx;
                    }
                }
                Action::None
            }
            MouseEventKind::ScrollUp => {
                self.cursor_pad = self.cursor_pad.saturating_sub(1);
                Action::None
            }
            MouseEventKind::ScrollDown => {
                self.cursor_pad = (self.cursor_pad + 1).min(NUM_PADS - 1);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
