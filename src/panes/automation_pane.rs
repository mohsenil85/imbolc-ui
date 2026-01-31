#![allow(dead_code)]

use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::automation::{AutomationLaneId, AutomationTarget, CurveType};
use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, AutomationAction, Color, InputEvent, Keymap, Pane, Style};

/// Block characters for mini value graph (8 levels)
const BLOCK_CHARS: [char; 8] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}',
    '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}',
];

/// Focus area within the automation pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutomationFocus {
    LaneList,
    Timeline,
}

/// Sub-mode for adding a new lane target
#[derive(Debug, Clone)]
enum TargetPickerState {
    Inactive,
    Active { options: Vec<AutomationTarget>, cursor: usize },
}

pub struct AutomationPane {
    keymap: Keymap,
    focus: AutomationFocus,
    // Timeline cursor
    cursor_tick: u32,
    cursor_value: f32,
    // Timeline viewport
    view_start_tick: u32,
    zoom_level: u8,
    snap_to_grid: bool,
    // Target picker sub-mode
    target_picker: TargetPickerState,
}

impl AutomationPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            focus: AutomationFocus::LaneList,
            cursor_tick: 0,
            cursor_value: 0.5,
            view_start_tick: 0,
            zoom_level: 3,
            snap_to_grid: true,
            target_picker: TargetPickerState::Inactive,
        }
    }

    fn ticks_per_cell(&self) -> u32 {
        match self.zoom_level {
            1 => 60,   // 1/8 beat
            2 => 120,  // 1/4 beat (sixteenth)
            3 => 240,  // 1/2 beat (eighth)
            4 => 480,  // 1 beat
            5 => 960,  // 2 beats
            _ => 480,
        }
    }

    fn snap_tick(&self, tick: u32) -> u32 {
        if self.snap_to_grid {
            let grid = self.ticks_per_cell();
            (tick / grid) * grid
        } else {
            tick
        }
    }

    /// Get the currently selected lane id
    fn selected_lane_id(&self, state: &AppState) -> Option<AutomationLaneId> {
        state.session.automation.selected().map(|l| l.id)
    }

    fn render_lane_list(&self, buf: &mut Buffer, area: RatatuiRect, state: &AppState) {
        if area.height < 2 || area.width < 10 {
            return;
        }

        let automation = &state.session.automation;

        // Filter lanes for the currently selected instrument (plus global lanes)
        let inst_id = state.instruments.selected_instrument().map(|i| i.id);
        let visible_lanes: Vec<(usize, &crate::state::automation::AutomationLane)> = automation
            .lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                match l.target.instrument_id() {
                    Some(id) => inst_id == Some(id),
                    None => true, // Global targets always visible
                }
            })
            .collect();

        if visible_lanes.is_empty() {
            let text = "(no automation lanes)";
            let style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
            let x = area.x + 1;
            let y = area.y;
            Paragraph::new(Line::from(Span::styled(text, style)))
                .render(RatatuiRect::new(x, y, text.len() as u16, 1), buf);
            return;
        }

        // Header
        let header = format!("{:<6} {:<16} {:>3} {:>4} {:<6}", "Lane", "Target", "En", "Pts", "Curve");
        let header_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        for (i, ch) in header.chars().enumerate() {
            if area.x + 1 + i as u16 >= area.x + area.width { break; }
            if let Some(cell) = buf.cell_mut((area.x + 1 + i as u16, area.y)) {
                cell.set_char(ch).set_style(header_style);
            }
        }

        for (vi, (global_idx, lane)) in visible_lanes.iter().enumerate() {
            let y = area.y + 1 + vi as u16;
            if y >= area.y + area.height { break; }

            let is_selected = automation.selected_lane == Some(*global_idx);
            let in_focus = self.focus == AutomationFocus::LaneList;

            let enabled_char = if lane.enabled { "x" } else { " " };
            let point_count = lane.points.len();
            let curve_name = if let Some(p) = lane.points.first() {
                match p.curve {
                    CurveType::Linear => "Linear",
                    CurveType::Exponential => "Exp",
                    CurveType::Step => "Step",
                    CurveType::SCurve => "SCurve",
                }
            } else {
                "Linear"
            };

            let short = lane.target.short_name();
            let name = lane.target.name();
            let line_text = format!(
                "{}{:<5} {:<16} [{}] {:>3} {:<6}",
                if is_selected { ">" } else { " " },
                short,
                name,
                enabled_char,
                point_count,
                curve_name
            );

            let style = if is_selected && in_focus {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
            } else if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::new(30, 30, 40)))
            } else if !lane.enabled {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::GRAY))
            };

            for (i, ch) in line_text.chars().enumerate() {
                let x = area.x + i as u16;
                if x >= area.x + area.width { break; }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch).set_style(style);
                }
            }
            // Fill remaining width for selected row
            if is_selected {
                for x in (area.x + line_text.len() as u16)..(area.x + area.width) {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(style);
                    }
                }
            }
        }
    }

    fn render_timeline(&self, buf: &mut Buffer, area: RatatuiRect, state: &AppState) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        let automation = &state.session.automation;
        let lane = match automation.selected() {
            Some(l) => l,
            None => {
                let text = "(select a lane)";
                let style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
                let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
                let y = area.y + area.height / 2;
                Paragraph::new(Line::from(Span::styled(text, style)))
                    .render(RatatuiRect::new(x, y, text.len() as u16, 1), buf);
                return;
            }
        };

        let tpc = self.ticks_per_cell();
        let graph_height = area.height.saturating_sub(2); // Reserve 1 for beat markers, 1 for status
        let graph_width = area.width;
        let graph_y = area.y;

        // Draw the value graph area
        let bg_style = ratatui::style::Style::from(Style::new().fg(Color::new(30, 30, 30)));
        let _beat_style = ratatui::style::Style::from(Style::new().fg(Color::new(45, 45, 45)));
        let bar_style = ratatui::style::Style::from(Style::new().fg(Color::new(55, 55, 55)));
        let in_focus = self.focus == AutomationFocus::Timeline;

        // Grid dots
        for col in 0..graph_width {
            let tick = self.view_start_tick + col as u32 * tpc;
            let is_bar = tick % 1920 == 0; // 4 beats
            let is_beat = tick % 480 == 0;

            for row in 0..graph_height {
                let y = graph_y + row;
                let x = area.x + col;
                if is_bar {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char('┊').set_style(bar_style);
                    }
                } else if is_beat && row == 0 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char('·').set_style(bg_style);
                    }
                }
            }
        }

        // Draw automation curve
        let curve_color = if lane.enabled { Color::CYAN } else { Color::DARK_GRAY };
        let curve_style = ratatui::style::Style::from(Style::new().fg(curve_color));
        let point_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(curve_color));

        if !lane.points.is_empty() && graph_height > 0 {
            for col in 0..graph_width {
                let tick = self.view_start_tick + col as u32 * tpc;
                if let Some(raw_value) = lane.value_at(tick) {
                    // Convert from actual range to normalized 0-1
                    let normalized = if lane.max_value > lane.min_value {
                        (raw_value - lane.min_value) / (lane.max_value - lane.min_value)
                    } else {
                        0.5
                    };
                    let row = ((1.0 - normalized) * (graph_height.saturating_sub(1)) as f32) as u16;
                    let y = graph_y + row;
                    let x = area.x + col;
                    if y < graph_y + graph_height {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            // Check if there's a point exactly at this tick
                            if lane.point_at(tick).is_some() {
                                cell.set_char('●').set_style(point_style);
                            } else {
                                cell.set_char('─').set_style(curve_style);
                            }
                        }
                    }
                }
            }
        }

        // Draw cursor
        if in_focus {
            let cursor_col = if self.cursor_tick >= self.view_start_tick {
                ((self.cursor_tick - self.view_start_tick) / tpc) as u16
            } else {
                0
            };
            let cursor_row = ((1.0 - self.cursor_value) * (graph_height.saturating_sub(1)) as f32) as u16;

            if cursor_col < graph_width {
                let x = area.x + cursor_col;

                // Vertical line at cursor tick
                for row in 0..graph_height {
                    let y = graph_y + row;
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        if row == cursor_row {
                            cell.set_char('◆').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG)),
                            );
                        } else if cell.symbol() == " " {
                            cell.set_char('│').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::new(50, 50, 60))),
                            );
                        }
                    }
                }
            }
        }

        // Beat markers row
        let marker_y = graph_y + graph_height;
        if marker_y < area.y + area.height {
            let marker_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
            for col in 0..graph_width {
                let tick = self.view_start_tick + col as u32 * tpc;
                if tick % 1920 == 0 {
                    // Bar number
                    let bar = tick / 1920 + 1;
                    let label = format!("B{}", bar);
                    for (j, ch) in label.chars().enumerate() {
                        let x = area.x + col + j as u16;
                        if x < area.x + graph_width {
                            if let Some(cell) = buf.cell_mut((x, marker_y)) {
                                cell.set_char(ch).set_style(marker_style);
                            }
                        }
                    }
                }
            }
        }

        // Status line
        let status_y = graph_y + graph_height + 1;
        if status_y < area.y + area.height {
            let curve_at_cursor = lane.point_at(self.cursor_tick)
                .map(|p| match p.curve {
                    CurveType::Linear => "Linear",
                    CurveType::Exponential => "Exp",
                    CurveType::Step => "Step",
                    CurveType::SCurve => "SCurve",
                })
                .unwrap_or("—");

            let rec_indicator = if state.automation_recording { " [REC]" } else { "" };
            let status = format!(
                " Tick:{:<6} Val:{:.2}  Curve:{}{}",
                self.cursor_tick,
                self.cursor_value,
                curve_at_cursor,
                rec_indicator,
            );

            let normal_style = ratatui::style::Style::from(Style::new().fg(Color::GRAY));
            let rec_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::RED));

            // Render status text
            for (i, ch) in status.chars().enumerate() {
                let x = area.x + i as u16;
                if x >= area.x + graph_width { break; }
                if let Some(cell) = buf.cell_mut((x, status_y)) {
                    // Use red style for [REC]
                    let is_rec_section = state.automation_recording
                        && i >= status.len() - 6;
                    let style = if is_rec_section { rec_style } else { normal_style };
                    cell.set_char(ch).set_style(style);
                }
            }
        }
    }

    fn render_target_picker(&self, buf: &mut Buffer, area: RatatuiRect) {
        if let TargetPickerState::Active { ref options, cursor } = self.target_picker {
            let picker_width = 30u16.min(area.width.saturating_sub(4));
            let picker_height = (options.len() as u16 + 2).min(area.height.saturating_sub(2));
            let picker_rect = center_rect(area, picker_width, picker_height);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Add Lane ")
                .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
                .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
            let inner = block.inner(picker_rect);
            // Clear background
            for y in picker_rect.y..picker_rect.y + picker_rect.height {
                for x in picker_rect.x..picker_rect.x + picker_rect.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ').set_style(
                            ratatui::style::Style::from(Style::new().bg(Color::new(20, 20, 30))),
                        );
                    }
                }
            }
            block.render(picker_rect, buf);

            for (i, target) in options.iter().enumerate() {
                let y = inner.y + i as u16;
                if y >= inner.y + inner.height { break; }

                let is_selected = i == cursor;
                let text = format!(
                    "{} {}",
                    if is_selected { ">" } else { " " },
                    target.name()
                );
                let style = if is_selected {
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
                } else {
                    ratatui::style::Style::from(Style::new().fg(Color::GRAY))
                };

                for (j, ch) in text.chars().enumerate() {
                    let x = inner.x + j as u16;
                    if x >= inner.x + inner.width { break; }
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(ch).set_style(style);
                    }
                }
                // Fill remaining for selected
                if is_selected {
                    for x in (inner.x + text.len() as u16)..(inner.x + inner.width) {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_style(style);
                        }
                    }
                }
            }
        }
    }

    /// Handle actions while the target picker is active
    fn handle_target_picker_action(&mut self, action: &str, _state: &AppState) -> Action {
        if let TargetPickerState::Active { ref options, ref mut cursor } = self.target_picker {
            match action {
                "up" | "prev" => {
                    if *cursor > 0 { *cursor -= 1; }
                    Action::None
                }
                "down" | "next" => {
                    if *cursor + 1 < options.len() { *cursor += 1; }
                    Action::None
                }
                "confirm" | "add_lane" => {
                    if let Some(target) = options.get(*cursor).cloned() {
                        self.target_picker = TargetPickerState::Inactive;
                        Action::Automation(AutomationAction::AddLane(target))
                    } else {
                        Action::None
                    }
                }
                "cancel" | "escape" => {
                    self.target_picker = TargetPickerState::Inactive;
                    Action::None
                }
                _ => Action::None,
            }
        } else {
            Action::None
        }
    }
}

impl Pane for AutomationPane {
    fn id(&self) -> &'static str {
        "automation"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        // If target picker is active, delegate to it
        if matches!(self.target_picker, TargetPickerState::Active { .. }) {
            return self.handle_target_picker_action(action, state);
        }

        match action {
            // Focus switching
            "switch_focus" => {
                self.focus = match self.focus {
                    AutomationFocus::LaneList => AutomationFocus::Timeline,
                    AutomationFocus::Timeline => AutomationFocus::LaneList,
                };
                Action::None
            }

            // Lane list actions
            "up" | "prev" => {
                if self.focus == AutomationFocus::LaneList {
                    Action::Automation(AutomationAction::SelectLane(-1))
                } else {
                    // Timeline: move value up
                    self.cursor_value = (self.cursor_value + 0.05).min(1.0);
                    Action::None
                }
            }
            "down" | "next" => {
                if self.focus == AutomationFocus::LaneList {
                    Action::Automation(AutomationAction::SelectLane(1))
                } else {
                    // Timeline: move value down
                    self.cursor_value = (self.cursor_value - 0.05).max(0.0);
                    Action::None
                }
            }
            "left" => {
                if self.focus == AutomationFocus::Timeline {
                    let tpc = self.ticks_per_cell();
                    self.cursor_tick = self.cursor_tick.saturating_sub(tpc);
                    // Scroll view if needed
                    if self.cursor_tick < self.view_start_tick {
                        self.view_start_tick = self.cursor_tick;
                    }
                }
                Action::None
            }
            "right" => {
                if self.focus == AutomationFocus::Timeline {
                    let tpc = self.ticks_per_cell();
                    self.cursor_tick += tpc;
                }
                Action::None
            }

            // Add lane
            "add_lane" => {
                let inst_id = state.instruments.selected_instrument().map(|i| i.id);
                let mut options: Vec<AutomationTarget> = Vec::new();
                if let Some(id) = inst_id {
                    options = AutomationTarget::targets_for_instrument(id);
                    // Add send targets
                    if let Some(inst) = state.instruments.selected_instrument() {
                        for (idx, _send) in inst.sends.iter().enumerate() {
                            options.push(AutomationTarget::SendLevel(id, idx));
                        }
                    }
                }
                // Add global targets
                for bus_id in 1..=8u8 {
                    options.push(AutomationTarget::BusLevel(bus_id));
                }
                options.push(AutomationTarget::Bpm);

                self.target_picker = TargetPickerState::Active { options, cursor: 0 };
                Action::None
            }

            // Remove lane
            "remove_lane" => {
                if let Some(id) = self.selected_lane_id(state) {
                    Action::Automation(AutomationAction::RemoveLane(id))
                } else {
                    Action::None
                }
            }

            // Toggle lane enabled
            "toggle_enabled" => {
                if let Some(id) = self.selected_lane_id(state) {
                    Action::Automation(AutomationAction::ToggleLaneEnabled(id))
                } else {
                    Action::None
                }
            }

            // Place/remove point (timeline)
            "place_point" => {
                if self.focus == AutomationFocus::Timeline {
                    if let Some(id) = self.selected_lane_id(state) {
                        let tick = self.snap_tick(self.cursor_tick);
                        let lane = state.session.automation.lane(id);
                        if let Some(lane) = lane {
                            if lane.point_at(tick).is_some() {
                                // Remove existing point
                                Action::Automation(AutomationAction::RemovePoint(id, tick))
                            } else {
                                // Add new point
                                Action::Automation(AutomationAction::AddPoint(id, tick, self.cursor_value))
                            }
                        } else {
                            Action::None
                        }
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }

            // Delete point at cursor
            "delete_point" => {
                if self.focus == AutomationFocus::Timeline {
                    if let Some(id) = self.selected_lane_id(state) {
                        let tick = self.snap_tick(self.cursor_tick);
                        Action::Automation(AutomationAction::RemovePoint(id, tick))
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }

            // Cycle curve type at cursor
            "cycle_curve" => {
                if self.focus == AutomationFocus::Timeline {
                    if let Some(id) = self.selected_lane_id(state) {
                        let tick = self.snap_tick(self.cursor_tick);
                        if let Some(lane) = state.session.automation.lane(id) {
                            if let Some(point) = lane.point_at(tick) {
                                let new_curve = match point.curve {
                                    CurveType::Linear => CurveType::Exponential,
                                    CurveType::Exponential => CurveType::Step,
                                    CurveType::Step => CurveType::SCurve,
                                    CurveType::SCurve => CurveType::Linear,
                                };
                                return Action::Automation(AutomationAction::SetCurveType(id, tick, new_curve));
                            }
                        }
                    }
                }
                Action::None
            }

            // Clear lane
            "clear_lane" => {
                if let Some(id) = self.selected_lane_id(state) {
                    Action::Automation(AutomationAction::ClearLane(id))
                } else {
                    Action::None
                }
            }

            // Toggle recording
            "toggle_recording" => {
                Action::Automation(AutomationAction::ToggleRecording)
            }

            // Zoom
            "zoom_in" => {
                self.zoom_level = self.zoom_level.saturating_sub(1).max(1);
                Action::None
            }
            "zoom_out" => {
                self.zoom_level = (self.zoom_level + 1).min(5);
                Action::None
            }

            // Home / End
            "home" => {
                self.cursor_tick = 0;
                self.view_start_tick = 0;
                Action::None
            }
            "end" => {
                // Jump to the last point in the selected lane
                if let Some(lane) = state.session.automation.selected() {
                    if let Some(last) = lane.points.last() {
                        self.cursor_tick = last.tick;
                        let tpc = self.ticks_per_cell();
                        self.view_start_tick = self.cursor_tick.saturating_sub(tpc * 10);
                    }
                }
                Action::None
            }

            // Play/stop (pass through to piano roll)
            "play_stop" => {
                Action::PianoRoll(crate::ui::PianoRollAction::PlayStop)
            }

            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let rect = center_rect(area, 100.min(area.width), 30.min(area.height));

        // Title
        let inst_name = state.instruments.selected_instrument()
            .map(|i| format!("Inst {} ({})", i.id, &i.name))
            .unwrap_or_else(|| "—".to_string());
        let title = format!(" Automation: {} ", inst_name);

        let border_color = Color::CYAN;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(ratatui::style::Style::from(Style::new().fg(border_color)))
            .title_style(ratatui::style::Style::from(Style::new().fg(border_color)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        if inner.height < 5 {
            return;
        }

        // Split inner area: top half for lane list, bottom half for timeline
        let lane_list_height = (inner.height / 3).max(3);
        let timeline_height = inner.height.saturating_sub(lane_list_height + 1);

        let lane_list_area = RatatuiRect::new(inner.x, inner.y, inner.width, lane_list_height);
        let separator_y = inner.y + lane_list_height;
        let timeline_area = RatatuiRect::new(inner.x, separator_y + 1, inner.width, timeline_height);

        // Render lane list
        self.render_lane_list(buf, lane_list_area, state);

        // Separator
        let sep_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let timeline_title = state.session.automation.selected()
            .map(|l| {
                let (min, max) = (l.min_value, l.max_value);
                format!("─ {} ({:.1}–{:.1}) ", l.target.name(), min, max)
            })
            .unwrap_or_else(|| "─".to_string());

        for x in inner.x..inner.x + inner.width {
            if let Some(cell) = buf.cell_mut((x, separator_y)) {
                cell.set_char('─').set_style(sep_style);
            }
        }
        // Overlay title on separator
        let title_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN));
        for (i, ch) in timeline_title.chars().enumerate() {
            let x = inner.x + 1 + i as u16;
            if x >= inner.x + inner.width { break; }
            if let Some(cell) = buf.cell_mut((x, separator_y)) {
                cell.set_char(ch).set_style(title_style);
            }
        }

        // Render timeline
        self.render_timeline(buf, timeline_area, state);

        // Render target picker overlay (if active)
        self.render_target_picker(buf, rect);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::ui::{InputEvent, KeyCode, Modifiers};

    fn dummy_event() -> InputEvent {
        InputEvent::new(KeyCode::Char('x'), Modifiers::default())
    }

    #[test]
    fn automation_pane_id() {
        let pane = AutomationPane::new(Keymap::new());
        assert_eq!(pane.id(), "automation");
    }

    #[test]
    fn switch_focus_toggles() {
        let mut pane = AutomationPane::new(Keymap::new());
        let state = AppState::new();
        assert_eq!(pane.focus, AutomationFocus::LaneList);

        pane.handle_action("switch_focus", &dummy_event(), &state);
        assert_eq!(pane.focus, AutomationFocus::Timeline);

        pane.handle_action("switch_focus", &dummy_event(), &state);
        assert_eq!(pane.focus, AutomationFocus::LaneList);
    }

    #[test]
    fn timeline_cursor_moves() {
        let mut pane = AutomationPane::new(Keymap::new());
        let state = AppState::new();
        pane.focus = AutomationFocus::Timeline;

        let start_tick = pane.cursor_tick;
        pane.handle_action("right", &dummy_event(), &state);
        assert!(pane.cursor_tick > start_tick);

        pane.handle_action("left", &dummy_event(), &state);
        assert_eq!(pane.cursor_tick, start_tick);
    }

    #[test]
    fn add_lane_opens_target_picker() {
        let mut pane = AutomationPane::new(Keymap::new());
        let state = AppState::new();
        pane.handle_action("add_lane", &dummy_event(), &state);
        assert!(matches!(pane.target_picker, TargetPickerState::Active { .. }));
    }
}
