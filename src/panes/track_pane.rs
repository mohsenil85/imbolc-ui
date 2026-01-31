use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, SourceType};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, Pane, Style};

fn source_color(source: SourceType) -> Color {
    match source {
        SourceType::Saw => Color::OSC_COLOR,
        SourceType::Sin => Color::OSC_COLOR,
        SourceType::Sqr => Color::OSC_COLOR,
        SourceType::Tri => Color::OSC_COLOR,
        SourceType::AudioIn => Color::AUDIO_IN_COLOR,
        SourceType::PitchedSampler => Color::SAMPLE_COLOR,
        SourceType::Kit => Color::KIT_COLOR,
        SourceType::BusIn => Color::BUS_IN_COLOR,
        SourceType::Custom(_) => Color::CUSTOM_COLOR,
        SourceType::Vst(_) => Color::VST_COLOR,
    }
}

pub struct TrackPane {
    keymap: Keymap,
}

impl TrackPane {
    pub fn new(keymap: Keymap) -> Self {
        Self { keymap }
    }
}

impl Default for TrackPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for TrackPane {
    fn id(&self) -> &'static str {
        "track"
    }

    fn handle_action(&mut self, _action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Track ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        if state.instruments.instruments.is_empty() {
            let text = "(no instruments)";
            let x = inner.x + (inner.width.saturating_sub(text.len() as u16)) / 2;
            let y = inner.y + inner.height / 2;
            Paragraph::new(Line::from(Span::styled(
                text,
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            )))
            .render(RatatuiRect::new(x, y, text.len() as u16, 1), buf);
            return;
        }

        // Layout: label column on the left, timeline area on the right
        let label_width: u16 = 20;
        let timeline_x = inner.x + label_width + 1;
        let timeline_width = inner.width.saturating_sub(label_width + 2);

        // Lane height: divide available space evenly, min 2 rows per lane
        let num_instruments = state.instruments.instruments.len();
        let lane_height = ((inner.height as usize) / num_instruments.max(1)).max(2).min(4) as u16;
        let max_visible = (inner.height / lane_height) as usize;

        // Scroll to keep selected visible
        let scroll_offset = state.instruments.selected
            .map(|s| if s >= max_visible { s - max_visible + 1 } else { 0 })
            .unwrap_or(0);

        let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));
        let separator_style = ratatui::style::Style::from(Style::new().fg(Color::new(40, 40, 40)));

        for (vi, i) in (scroll_offset..num_instruments).enumerate() {
            if vi >= max_visible {
                break;
            }
            let instrument = &state.instruments.instruments[i];
            let is_selected = state.instruments.selected == Some(i);
            let lane_y = inner.y + (vi as u16) * lane_height;

            if lane_y + lane_height > inner.y + inner.height {
                break;
            }

            let source_c = source_color(instrument.source);

            // Selection indicator
            if is_selected {
                if let Some(cell) = buf.cell_mut((inner.x, lane_y)) {
                    cell.set_char('>').set_style(
                        ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                    );
                }
            }

            // Instrument number + name
            let num_str = format!("{:>2} ", i + 1);
            let name_str = format!("{}", &instrument.name[..instrument.name.len().min(14)]);

            let num_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            let name_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE))
            };

            let label_line = Line::from(vec![
                Span::styled(num_str, num_style),
                Span::styled(name_str, name_style),
            ]);
            Paragraph::new(label_line).render(
                RatatuiRect::new(inner.x + 1, lane_y, label_width, 1), buf,
            );

            // Source type on second line of lane (if space)
            if lane_height > 2 {
                let src_str = format!("   {}", instrument.source.name());
                let src_style = if is_selected {
                    ratatui::style::Style::from(Style::new().fg(source_c).bg(Color::SELECTION_BG))
                } else {
                    ratatui::style::Style::from(Style::new().fg(source_c))
                };
                Paragraph::new(Line::from(Span::styled(src_str, src_style))).render(
                    RatatuiRect::new(inner.x + 1, lane_y + 1, label_width, 1), buf,
                );
            }

            // Fill label area bg for selected
            if is_selected {
                for row in 0..lane_height {
                    let y = lane_y + row;
                    if y >= inner.y + inner.height { break; }
                    for x in (inner.x + 1)..timeline_x {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            if cell.symbol() == " " {
                                cell.set_style(sel_bg);
                            }
                        }
                    }
                }
            }

            // Separator between label and timeline
            for row in 0..lane_height {
                let y = lane_y + row;
                if y >= inner.y + inner.height { break; }
                if let Some(cell) = buf.cell_mut((inner.x + label_width, y)) {
                    cell.set_char('│').set_style(
                        ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
                    );
                }
            }

            // Timeline area: empty lane with bar markers
            let lane_color = if is_selected { source_c } else { Color::new(35, 35, 35) };
            let lane_style = ratatui::style::Style::from(Style::new().fg(lane_color));
            let bar_style = ratatui::style::Style::from(Style::new().fg(Color::new(50, 50, 50)));

            for col in 0..timeline_width {
                let x = timeline_x + col;
                // Bar lines every ~16 chars
                let is_bar = col % 16 == 0;
                let is_beat = col % 4 == 0;

                for row in 0..lane_height {
                    let y = lane_y + row;
                    if y >= inner.y + inner.height { break; }
                    if is_bar {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('┊').set_style(bar_style);
                        }
                    } else if is_beat && row == 0 {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('·').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::new(30, 30, 30))),
                            );
                        }
                    }
                }

                // Draw a thin line in the middle of the lane to indicate the track
                let mid_y = lane_y + lane_height / 2;
                if mid_y < inner.y + inner.height && !is_bar {
                    if let Some(cell) = buf.cell_mut((x, mid_y)) {
                        cell.set_char('─').set_style(lane_style);
                    }
                }
            }

            // Horizontal separator below each lane
            let sep_y = lane_y + lane_height - 1;
            if sep_y < inner.y + inner.height && vi < max_visible - 1 && i < num_instruments - 1 {
                for x in inner.x..(inner.x + inner.width) {
                    if let Some(cell) = buf.cell_mut((x, sep_y)) {
                        cell.set_char('─').set_style(separator_style);
                    }
                }
            }
        }

        // Bar numbers along the bottom
        let footer_y = inner.y + inner.height - 1;
        for col in 0..timeline_width {
            if col % 16 == 0 {
                let bar_num = (col / 16) + 1;
                let label = format!("{}", bar_num);
                let x = timeline_x + col;
                let label_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
                for (j, ch) in label.chars().enumerate() {
                    if x + (j as u16) < inner.x + inner.width {
                        if let Some(cell) = buf.cell_mut((x + j as u16, footer_y)) {
                            cell.set_char(ch).set_style(label_style);
                        }
                    }
                }
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
