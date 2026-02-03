use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, SourceType};
use crate::state::arrangement::PlayMode;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, ArrangementAction, Color, InputEvent, Keymap, Pane, Style};

fn source_color(source: SourceType) -> Color {
    match source {
        SourceType::Saw | SourceType::Sin | SourceType::Sqr | SourceType::Tri
        | SourceType::Noise | SourceType::Pulse | SourceType::SuperSaw | SourceType::Sync
        | SourceType::Ring | SourceType::FBSin | SourceType::FM | SourceType::PhaseMod
        | SourceType::Pluck | SourceType::Formant | SourceType::Gendy | SourceType::Chaos
        | SourceType::Additive | SourceType::Wavetable | SourceType::Granular => Color::OSC_COLOR,
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
    /// Index into current instrument's clips list for placement selection
    selected_clip_index: usize,
}

impl TrackPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            selected_clip_index: 0,
        }
    }

    fn ticks_per_bar(&self, state: &AppState) -> u32 {
        let (beats, _) = state.session.time_signature;
        beats as u32 * 480
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

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        let arr = &state.session.arrangement;
        let num_instruments = state.instruments.instruments.len();
        if num_instruments == 0 {
            return Action::None;
        }

        let lane = arr.selected_lane.min(num_instruments.saturating_sub(1));
        let instrument_id = state.instruments.instruments[lane].id;

        match action {
            "lane_up" => {
                if lane > 0 {
                    Action::Arrangement(ArrangementAction::SelectLane(lane - 1))
                } else {
                    Action::None
                }
            }
            "lane_down" => {
                if lane + 1 < num_instruments {
                    Action::Arrangement(ArrangementAction::SelectLane(lane + 1))
                } else {
                    Action::None
                }
            }
            "cursor_left" => Action::Arrangement(ArrangementAction::MoveCursor(-1)),
            "cursor_right" => Action::Arrangement(ArrangementAction::MoveCursor(1)),
            "cursor_home" => {
                // Jump to tick 0
                let delta = -(arr.cursor_tick as i32 / arr.ticks_per_col.max(1) as i32);
                Action::Arrangement(ArrangementAction::MoveCursor(delta))
            }
            "cursor_end" => {
                let end = arr.arrangement_length();
                if end > arr.cursor_tick {
                    let delta = (end - arr.cursor_tick) as i32 / arr.ticks_per_col.max(1) as i32;
                    Action::Arrangement(ArrangementAction::MoveCursor(delta))
                } else {
                    Action::None
                }
            }
            "new_clip" => {
                Action::Arrangement(ArrangementAction::CaptureClipFromPianoRoll {
                    instrument_id,
                })
            }
            "new_empty_clip" => {
                let tpb = self.ticks_per_bar(state);
                Action::Arrangement(ArrangementAction::CreateClip {
                    instrument_id,
                    length_ticks: tpb,
                })
            }
            "place_clip" => {
                // Place the selected clip at cursor position
                let clips = arr.clips_for_instrument(instrument_id);
                if clips.is_empty() {
                    return Action::None;
                }
                let idx = self.selected_clip_index.min(clips.len().saturating_sub(1));
                let clip_id = clips[idx].id;
                Action::Arrangement(ArrangementAction::PlaceClip {
                    clip_id,
                    instrument_id,
                    start_tick: arr.cursor_tick,
                })
            }
            "edit_clip" => {
                // Edit clip under cursor
                if let Some(placement) = arr.placement_at(instrument_id, arr.cursor_tick) {
                    Action::Arrangement(ArrangementAction::EnterClipEdit(placement.clip_id))
                } else {
                    Action::None
                }
            }
            "delete" => {
                // Delete selected placement
                if let Some(placement) = arr.placement_at(instrument_id, arr.cursor_tick) {
                    Action::Arrangement(ArrangementAction::RemovePlacement(placement.id))
                } else {
                    Action::None
                }
            }
            "delete_clip" => {
                // Delete clip and all placements
                let clips = arr.clips_for_instrument(instrument_id);
                if clips.is_empty() {
                    return Action::None;
                }
                let idx = self.selected_clip_index.min(clips.len().saturating_sub(1));
                let clip_id = clips[idx].id;
                Action::Arrangement(ArrangementAction::DeleteClip(clip_id))
            }
            "duplicate" => {
                if let Some(placement) = arr.placement_at(instrument_id, arr.cursor_tick) {
                    Action::Arrangement(ArrangementAction::DuplicatePlacement(placement.id))
                } else {
                    Action::None
                }
            }
            "toggle_mode" => Action::Arrangement(ArrangementAction::TogglePlayMode),
            "play_stop" => Action::Arrangement(ArrangementAction::PlayStop),
            "move_left" => {
                if let Some(placement) = arr.placement_at(instrument_id, arr.cursor_tick) {
                    let new_start = placement.start_tick.saturating_sub(arr.ticks_per_col);
                    Action::Arrangement(ArrangementAction::MovePlacement {
                        placement_id: placement.id,
                        new_start_tick: new_start,
                    })
                } else {
                    Action::None
                }
            }
            "move_right" => {
                if let Some(placement) = arr.placement_at(instrument_id, arr.cursor_tick) {
                    let new_start = placement.start_tick + arr.ticks_per_col;
                    Action::Arrangement(ArrangementAction::MovePlacement {
                        placement_id: placement.id,
                        new_start_tick: new_start,
                    })
                } else {
                    Action::None
                }
            }
            "zoom_in" => Action::Arrangement(ArrangementAction::ZoomIn),
            "zoom_out" => Action::Arrangement(ArrangementAction::ZoomOut),
            "select_next_placement" => {
                let placements = arr.placements_for_instrument(instrument_id);
                if placements.is_empty() {
                    return Action::None;
                }
                let next = match arr.selected_placement {
                    Some(i) => {
                        let next_idx = i + 1;
                        if next_idx < placements.len() { Some(next_idx) } else { Some(0) }
                    }
                    None => Some(0),
                };
                Action::Arrangement(ArrangementAction::SelectPlacement(next))
            }
            "select_prev_placement" => {
                let placements = arr.placements_for_instrument(instrument_id);
                if placements.is_empty() {
                    return Action::None;
                }
                let prev = match arr.selected_placement {
                    Some(0) => Some(placements.len().saturating_sub(1)),
                    Some(i) => Some(i - 1),
                    None => Some(0),
                };
                Action::Arrangement(ArrangementAction::SelectPlacement(prev))
            }
            "select_next_clip" => {
                let clips = arr.clips_for_instrument(instrument_id);
                if !clips.is_empty() {
                    self.selected_clip_index = (self.selected_clip_index + 1) % clips.len();
                }
                Action::None
            }
            "select_prev_clip" => {
                let clips = arr.clips_for_instrument(instrument_id);
                if !clips.is_empty() {
                    if self.selected_clip_index == 0 {
                        self.selected_clip_index = clips.len() - 1;
                    } else {
                        self.selected_clip_index -= 1;
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 97, 29);
        let arr = &state.session.arrangement;
        let ticks_per_col = arr.ticks_per_col.max(1);

        // Mode indicator for title
        let mode_str = match arr.play_mode {
            PlayMode::Pattern => "Pattern",
            PlayMode::Song => "Song",
        };
        let title = format!(" Track [{}] ", mode_str);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.as_str())
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

        // Layout: header(1) + lanes + footer(2)
        let label_width: u16 = 20;
        let timeline_x = inner.x + label_width + 1;
        let timeline_width = inner.width.saturating_sub(label_width + 2);
        let header_height: u16 = 1;
        let footer_height: u16 = 2;
        let lanes_area_y = inner.y + header_height;
        let lanes_area_height = inner.height.saturating_sub(header_height + footer_height);

        let num_instruments = state.instruments.instruments.len();
        let lane_height: u16 = 2;
        let max_visible = (lanes_area_height / lane_height) as usize;

        // Scroll to keep selected lane visible
        let selected_lane = arr.selected_lane.min(num_instruments.saturating_sub(1));
        let scroll = if selected_lane >= max_visible {
            selected_lane - max_visible + 1
        } else {
            0
        };

        let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));
        let bar_line_style = ratatui::style::Style::from(Style::new().fg(Color::new(50, 50, 50)));
        let _separator_style = ratatui::style::Style::from(Style::new().fg(Color::new(40, 40, 40)));

        // Compute bar spacing in columns
        let (beats_per_bar, _) = state.session.time_signature;
        let ticks_per_bar = beats_per_bar as u32 * 480;
        let cols_per_bar = ticks_per_bar / ticks_per_col;
        let cols_per_beat = 480 / ticks_per_col;

        // --- Header: bar numbers ---
        let header_y = inner.y;
        for col in 0..timeline_width as u32 {
            let tick = arr.view_start_tick + col * ticks_per_col;
            if cols_per_bar > 0 && (tick % ticks_per_bar) < ticks_per_col {
                let bar_num = tick / ticks_per_bar + 1;
                let label = format!("{}", bar_num);
                let x = timeline_x + col as u16;
                let label_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
                for (j, ch) in label.chars().enumerate() {
                    if x + (j as u16) < inner.x + inner.width {
                        if let Some(cell) = buf.cell_mut((x + j as u16, header_y)) {
                            cell.set_char(ch).set_style(label_style);
                        }
                    }
                }
            }
        }

        // --- Instrument lanes ---
        for (vi, i) in (scroll..num_instruments).enumerate() {
            if vi >= max_visible {
                break;
            }
            let instrument = &state.instruments.instruments[i];
            let is_selected = i == selected_lane;
            let lane_y = lanes_area_y + (vi as u16) * lane_height;

            if lane_y + lane_height > lanes_area_y + lanes_area_height {
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
            let name_str = &instrument.name[..instrument.name.len().min(11)];
            let src_short = format!(" {}", instrument.source.name());

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
            let src_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(source_c).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(source_c))
            };

            // Line 1: number + name
            let label_line = Line::from(vec![
                Span::styled(num_str, num_style),
                Span::styled(name_str, name_style),
            ]);
            Paragraph::new(label_line).render(
                RatatuiRect::new(inner.x + 1, lane_y, label_width, 1), buf,
            );

            // Line 2: source type
            Paragraph::new(Line::from(Span::styled(
                &src_short[..src_short.len().min(label_width as usize)],
                src_style,
            ))).render(
                RatatuiRect::new(inner.x + 1, lane_y + 1, label_width, 1), buf,
            );

            // Fill label area bg for selected
            if is_selected {
                for row in 0..lane_height {
                    let y = lane_y + row;
                    if y >= lanes_area_y + lanes_area_height { break; }
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
                if y >= lanes_area_y + lanes_area_height { break; }
                if let Some(cell) = buf.cell_mut((inner.x + label_width, y)) {
                    cell.set_char('|').set_style(
                        ratatui::style::Style::from(Style::new().fg(Color::GRAY)),
                    );
                }
            }

            // Timeline area: bar/beat lines + clip blocks
            let inst_id = instrument.id;

            // Draw bar/beat lines
            for col in 0..timeline_width as u32 {
                let tick = arr.view_start_tick + col * ticks_per_col;
                let x = timeline_x + col as u16;
                let is_bar = cols_per_bar > 0 && (tick % ticks_per_bar) < ticks_per_col;
                let is_beat = cols_per_beat > 0 && (tick % 480) < ticks_per_col;

                for row in 0..lane_height {
                    let y = lane_y + row;
                    if y >= lanes_area_y + lanes_area_height { break; }
                    if is_bar {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('|').set_style(bar_line_style);
                        }
                    } else if is_beat && row == 0 {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('.').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::new(30, 30, 30))),
                            );
                        }
                    }
                }
            }

            // Draw clip placements for this instrument
            let placements = arr.placements_for_instrument(inst_id);
            for placement in &placements {
                if let Some(clip) = arr.clip(placement.clip_id) {
                    let _eff_len = placement.effective_length(clip);
                    let start_col = placement.start_tick.saturating_sub(arr.view_start_tick) / ticks_per_col;
                    let end_col = placement.end_tick(clip).saturating_sub(arr.view_start_tick) / ticks_per_col;

                    // Skip if entirely off-screen
                    if placement.end_tick(clip) <= arr.view_start_tick {
                        continue;
                    }
                    if placement.start_tick >= arr.view_start_tick + (timeline_width as u32) * ticks_per_col {
                        continue;
                    }

                    let vis_start = if placement.start_tick < arr.view_start_tick { 0 } else { start_col as u16 };
                    let vis_end = (end_col as u16).min(timeline_width);

                    if vis_start >= vis_end {
                        continue;
                    }

                    let clip_bg = source_c;
                    let clip_style = ratatui::style::Style::from(
                        Style::new().fg(Color::BLACK).bg(clip_bg),
                    );
                    let sel_clip_style = ratatui::style::Style::from(
                        Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold(),
                    );

                    let is_placement_selected = arr.selected_placement
                        .and_then(|idx| arr.placements.get(idx))
                        .map(|p| p.id == placement.id)
                        .unwrap_or(false);

                    let style = if is_placement_selected { sel_clip_style } else { clip_style };

                    // Render clip block
                    let block_width = vis_end - vis_start;
                    let name = &clip.name;
                    let display_name: String = if name.len() > block_width as usize {
                        name[..block_width as usize].to_string()
                    } else {
                        let padding = block_width as usize - name.len();
                        let left_pad = 0;
                        let right_pad = padding;
                        format!("{}{}{}", " ".repeat(left_pad), name, " ".repeat(right_pad))
                    };

                    // Fill both rows of the lane
                    for row in 0..lane_height {
                        let y = lane_y + row;
                        if y >= lanes_area_y + lanes_area_height { break; }
                        let x = timeline_x + vis_start;
                        if row == 0 {
                            // Name on first row
                            for (j, ch) in display_name.chars().enumerate() {
                                if x + (j as u16) < timeline_x + timeline_width {
                                    if let Some(cell) = buf.cell_mut((x + j as u16, y)) {
                                        cell.set_char(ch).set_style(style);
                                    }
                                }
                            }
                        } else {
                            // Fill second row
                            for j in 0..block_width {
                                if x + j < timeline_x + timeline_width {
                                    if let Some(cell) = buf.cell_mut((x + j, y)) {
                                        cell.set_char(' ').set_style(style);
                                    }
                                }
                            }
                        }
                    }

                    // Clip boundary markers
                    if vis_start > 0 || placement.start_tick >= arr.view_start_tick {
                        let x = timeline_x + vis_start;
                        for row in 0..lane_height {
                            let y = lane_y + row;
                            if y >= lanes_area_y + lanes_area_height { break; }
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char('[').set_style(style);
                            }
                        }
                    }
                    if vis_end <= timeline_width && vis_end > vis_start {
                        let x = timeline_x + vis_end - 1;
                        for row in 0..lane_height {
                            let y = lane_y + row;
                            if y >= lanes_area_y + lanes_area_height { break; }
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(']').set_style(style);
                            }
                        }
                    }
                }
            }

            // Horizontal separator below each lane
            if vi + 1 < max_visible && i + 1 < num_instruments {
                let sep_y = lane_y + lane_height;
                if sep_y < lanes_area_y + lanes_area_height {
                    for x in (inner.x + label_width + 1)..(inner.x + inner.width) {
                        if let Some(cell) = buf.cell_mut((x, sep_y)) {
                            if cell.symbol() == " " {
                                cell.set_char('-').set_style(_separator_style);
                            }
                        }
                    }
                }
            }
        }

        // --- Playhead ---
        let playhead_tick = state.session.piano_roll.playhead;
        if playhead_tick >= arr.view_start_tick {
            let playhead_col = (playhead_tick - arr.view_start_tick) / ticks_per_col;
            if (playhead_col as u16) < timeline_width {
                let x = timeline_x + playhead_col as u16;
                let ph_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bold());
                for y in lanes_area_y..(lanes_area_y + lanes_area_height) {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char('|').set_style(ph_style);
                    }
                }
            }
        }

        // --- Cursor ---
        if arr.cursor_tick >= arr.view_start_tick {
            let cursor_col = (arr.cursor_tick - arr.view_start_tick) / ticks_per_col;
            if (cursor_col as u16) < timeline_width {
                let x = timeline_x + cursor_col as u16;
                let lane_y = lanes_area_y + ((selected_lane - scroll.min(selected_lane)) as u16) * lane_height;
                let cursor_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN));
                for row in 0..lane_height {
                    let y = lane_y + row;
                    if y < lanes_area_y + lanes_area_height {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            // Only set cursor if cell is empty/space
                            if cell.symbol() == " " {
                                cell.set_char('|').set_style(cursor_style);
                            }
                        }
                    }
                }
            }
        }

        // --- Footer ---
        let footer_y = inner.y + inner.height - 2;

        // Line 1: key hints
        let hints = "n:new  p:place  Enter:edit  d:del  m:mode  Space:play  z/x:zoom";
        let hint_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        Paragraph::new(Line::from(Span::styled(hints, hint_style))).render(
            RatatuiRect::new(inner.x + 1, footer_y, inner.width.saturating_sub(2), 1), buf,
        );

        // Line 2: cursor position + selected clip info
        let bar = arr.cursor_tick / ticks_per_bar + 1;
        let beat = (arr.cursor_tick % ticks_per_bar) / 480 + 1;
        let inst_id = state.instruments.instruments[selected_lane].id;
        let clips = arr.clips_for_instrument(inst_id);
        let clip_info = if clips.is_empty() {
            "No clips".to_string()
        } else {
            let idx = self.selected_clip_index.min(clips.len().saturating_sub(1));
            format!("Clip: {} [{}/{}]", clips[idx].name, idx + 1, clips.len())
        };

        let pos_str = format!("Bar {} Beat {}  |  {}", bar, beat, clip_info);
        let pos_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
        Paragraph::new(Line::from(Span::styled(pos_str, pos_style))).render(
            RatatuiRect::new(inner.x + 1, footer_y + 1, inner.width.saturating_sub(2), 1), buf,
        );
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
