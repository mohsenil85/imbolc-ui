use std::any::Any;

use crate::state::drum_sequencer::NUM_PADS;
use crate::state::AppState;
use crate::ui::{Action, Color, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, SequencerAction, Style};

pub struct SequencerPane {
    keymap: Keymap,
    cursor_pad: usize,
    cursor_step: usize,
    view_start_step: usize,
}

impl SequencerPane {
    pub fn new() -> Self {
        Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Up, "up", "Previous pad")
                .bind_key(KeyCode::Down, "down", "Next pad")
                .bind_key(KeyCode::Left, "left", "Previous step")
                .bind_key(KeyCode::Right, "right", "Next step")
                .bind('j', "down", "Next pad")
                .bind('k', "up", "Previous pad")
                .bind('h', "left", "Previous step")
                .bind('l', "right", "Next step")
                .bind_key(KeyCode::Enter, "toggle", "Toggle step")
                .bind(' ', "play_stop", "Play/stop")
                .bind('s', "load_sample", "Load sample for pad")
                .bind('c', "clear_pad", "Clear pad steps")
                .bind_ctrl('c', "clear_pattern", "Clear pattern")
                .bind('[', "prev_pattern", "Previous pattern")
                .bind(']', "next_pattern", "Next pattern")
                .bind('{', "cycle_length", "Cycle pattern length"),
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
        Self::new()
    }
}

impl Pane for SequencerPane {
    fn id(&self) -> &'static str {
        "sequencer"
    }

    fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action {
        let pattern_length = state.session.drum_sequencer.pattern().length;

        // Manual shift checks before keymap lookup
        if event.modifiers.shift {
            match event.key {
                KeyCode::Up => {
                    return Action::Sequencer(SequencerAction::AdjustVelocity(
                        self.cursor_pad,
                        self.cursor_step,
                        10,
                    ));
                }
                KeyCode::Down => {
                    return Action::Sequencer(SequencerAction::AdjustVelocity(
                        self.cursor_pad,
                        self.cursor_step,
                        -10,
                    ));
                }
                KeyCode::Left => {
                    return Action::Sequencer(SequencerAction::AdjustPadLevel(
                        self.cursor_pad,
                        -0.05,
                    ));
                }
                KeyCode::Right => {
                    return Action::Sequencer(SequencerAction::AdjustPadLevel(
                        self.cursor_pad,
                        0.05,
                    ));
                }
                _ => {}
            }
        }

        match self.keymap.lookup(&event) {
            Some("up") => {
                self.cursor_pad = self.cursor_pad.saturating_sub(1);
                Action::None
            }
            Some("down") => {
                self.cursor_pad = (self.cursor_pad + 1).min(NUM_PADS - 1);
                Action::None
            }
            Some("left") => {
                self.cursor_step = self.cursor_step.saturating_sub(1);
                Action::None
            }
            Some("right") => {
                self.cursor_step = (self.cursor_step + 1).min(pattern_length - 1);
                Action::None
            }
            Some("toggle") => Action::Sequencer(SequencerAction::ToggleStep(
                self.cursor_pad,
                self.cursor_step,
            )),
            Some("play_stop") => Action::Sequencer(SequencerAction::PlayStop),
            Some("load_sample") => {
                Action::Sequencer(SequencerAction::LoadSample(self.cursor_pad))
            }
            Some("clear_pad") => Action::Sequencer(SequencerAction::ClearPad(self.cursor_pad)),
            Some("clear_pattern") => Action::Sequencer(SequencerAction::ClearPattern),
            Some("prev_pattern") => Action::Sequencer(SequencerAction::PrevPattern),
            Some("next_pattern") => Action::Sequencer(SequencerAction::NextPattern),
            Some("cycle_length") => Action::Sequencer(SequencerAction::CyclePatternLength),
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, state: &AppState) {
        let (width, height) = g.size();
        let box_width: u16 = 97;
        let box_height: u16 = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        let seq = &state.session.drum_sequencer;
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
        g.set_style(Style::new().fg(Color::ORANGE));
        g.draw_box(rect, Some(" Drum Sequencer "));

        let cx = rect.x + 2;
        let cy = rect.y + 1;

        // Header line: Pattern, Length, BPM, Play/Stop
        let pattern_label = match seq.current_pattern {
            0 => "A",
            1 => "B",
            2 => "C",
            3 => "D",
            _ => "?",
        };
        let play_label = if seq.playing { "PLAY" } else { "STOP" };
        let play_color = if seq.playing {
            Color::GREEN
        } else {
            Color::GRAY
        };

        g.set_style(Style::new().fg(Color::WHITE).bold());
        g.put_str(cx, cy, &format!("Pattern {}", pattern_label));

        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(cx + 12, cy, &format!("Length: {}", pattern.length));

        let bpm = state.session.piano_roll.bpm;
        g.put_str(cx + 24, cy, &format!("BPM: {:.0}", bpm));

        g.set_style(Style::new().fg(play_color).bold());
        g.put_str(cx + 36, cy, play_label);

        // Step number header
        let header_y = cy + 2;
        let label_width: u16 = 11; // pad label column width
        let step_col_start = cx + label_width;

        g.set_style(Style::new().fg(Color::DARK_GRAY));
        for i in 0..steps_shown {
            let step_num = view_start + i + 1;
            let x = step_col_start + (i as u16) * 3;
            if step_num < 10 {
                g.put_str(x + 1, header_y, &format!("{}", step_num));
            } else {
                g.put_str(x, header_y, &format!("{:2}", step_num));
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
                let name = if pad.name.len() > 6 {
                    &pad.name[..6]
                } else {
                    &pad.name
                };
                format!("{:>2} {:<6} ", pad_idx + 1, name)
            };

            if is_cursor_row {
                g.set_style(Style::new().fg(Color::WHITE).bold());
            } else {
                g.set_style(Style::new().fg(Color::GRAY));
            }
            g.put_str(cx, y, &label);

            // Steps
            for i in 0..steps_shown {
                let step_idx = view_start + i;
                let x = step_col_start + (i as u16) * 3;
                let is_cursor = is_cursor_row && step_idx == self.cursor_step;
                let is_playhead = seq.playing && step_idx == seq.current_step;

                let step = &pattern.steps[pad_idx][step_idx];
                let is_beat = step_idx % 4 == 0;

                // Determine cell style
                let (fg, bg) = if is_cursor {
                    if step.active {
                        (Color::BLACK, Color::WHITE)
                    } else {
                        (Color::WHITE, Color::SELECTION_BG)
                    }
                } else if is_playhead {
                    if step.active {
                        (Color::BLACK, Color::GREEN)
                    } else {
                        (Color::GREEN, Color::new(20, 50, 20))
                    }
                } else if step.active {
                    // Velocity-mapped color
                    let intensity = (step.velocity as f32 / 127.0 * 200.0) as u8 + 55;
                    (Color::new(intensity, intensity / 3, 0), Color::BLACK)
                } else if is_beat {
                    (Color::new(60, 60, 60), Color::BLACK)
                } else {
                    (Color::new(40, 40, 40), Color::BLACK)
                };

                g.set_style(Style::new().fg(fg).bg(bg));
                let ch = if step.active { " \u{2588} " } else { " \u{00B7} " };
                g.put_str(x, y, ch);
            }

            // Reset bg
            g.set_style(Style::new().fg(Color::WHITE));
        }

        // Pad detail line
        let detail_y = grid_y + NUM_PADS as u16 + 1;
        let pad = &seq.pads[self.cursor_pad];

        g.set_style(Style::new().fg(Color::ORANGE).bold());
        g.put_str(cx, detail_y, &format!("Pad {:>2}", self.cursor_pad + 1));

        g.set_style(Style::new().fg(Color::WHITE));
        if pad.name.is_empty() {
            g.put_str(cx + 8, detail_y, "(no sample)");
        } else {
            let display_name = if pad.name.len() > 20 {
                &pad.name[..20]
            } else {
                &pad.name
            };
            g.put_str(cx + 8, detail_y, display_name);
        }

        // Level bar
        let level_x = cx + 32;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(level_x, detail_y, "Level:");

        let bar_x = level_x + 7;
        let bar_width = 10;
        let filled = (pad.level * bar_width as f32) as usize;
        for i in 0..bar_width {
            if i < filled {
                g.set_style(Style::new().fg(Color::ORANGE));
                g.put_char(bar_x + i as u16, detail_y, '\u{2588}');
            } else {
                g.set_style(Style::new().fg(Color::new(40, 40, 40)));
                g.put_char(bar_x + i as u16, detail_y, '\u{2591}');
            }
        }

        // Velocity of current step
        let step = &pattern.steps[self.cursor_pad][self.cursor_step];
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            bar_x + bar_width as u16 + 2,
            detail_y,
            &format!("Vel: {}", step.velocity),
        );

        // Scroll indicator
        if pattern.length > visible {
            let scroll_x = rect.x + rect.width - 12;
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            g.put_str(
                scroll_x,
                detail_y,
                &format!(
                    "{}-{}/{}",
                    view_start + 1,
                    view_start + steps_shown,
                    pattern.length
                ),
            );
        }

        // Help line
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            cx,
            help_y,
            "Enter:toggle  Space:play/stop  s:sample  c:clear  []:pattern  {:length  ?:help",
        );
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
