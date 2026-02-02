use std::any::Any;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::state::recent_projects::RecentProjects;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, NavAction, Pane, SessionAction, Style};

pub struct ProjectBrowserPane {
    keymap: Keymap,
    entries: Vec<ProjectEntry>,
    selected: usize,
}

struct ProjectEntry {
    name: String,
    path: PathBuf,
    last_opened: SystemTime,
}

impl ProjectBrowserPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            entries: Vec::new(),
            selected: 0,
        }
    }

    /// Refresh the project list from disk
    pub fn refresh(&mut self) {
        let recent = RecentProjects::load();
        self.entries = recent.entries.into_iter().map(|e| ProjectEntry {
            name: e.name,
            path: e.path,
            last_opened: e.last_opened,
        }).collect();
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    fn format_time_ago(time: SystemTime) -> String {
        let now = SystemTime::now();
        let elapsed = now.duration_since(time).unwrap_or_default();
        let secs = elapsed.as_secs();
        if secs < 60 { return "just now".to_string(); }
        if secs < 3600 { return format!("{} min ago", secs / 60); }
        if secs < 86400 { return format!("{} hours ago", secs / 3600); }
        if secs < 604800 { return format!("{} days ago", secs / 86400); }
        // Fallback to date
        let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let days = since_epoch / 86400;
        let years = 1970 + days / 365;
        format!("{}", years)
    }
}

impl Pane for ProjectBrowserPane {
    fn id(&self) -> &'static str {
        "project_browser"
    }

    fn on_enter(&mut self, _state: &AppState) {
        self.refresh();
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        match action {
            "close" => Action::Nav(NavAction::PopPane),
            "up" => {
                if self.selected > 0 { self.selected -= 1; }
                Action::None
            }
            "down" => {
                if self.selected + 1 < self.entries.len() { self.selected += 1; }
                Action::None
            }
            "select" => {
                if let Some(entry) = self.entries.get(self.selected) {
                    let path = entry.path.clone();
                    if state.dirty {
                        // Dirty check handled by caller — for now just load directly
                        // The confirm pane intercept happens in global_actions
                        return Action::Session(SessionAction::LoadFrom(path));
                    }
                    return Action::Session(SessionAction::LoadFrom(path));
                }
                Action::None
            }
            "new_project" => {
                Action::Session(SessionAction::NewProject)
            }
            "delete_entry" => {
                if let Some(entry) = self.entries.get(self.selected) {
                    let path = entry.path.clone();
                    let mut recent = RecentProjects::load();
                    recent.remove(&path);
                    recent.save();
                    self.refresh();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_raw_input(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        match event.key {
            crate::ui::KeyCode::Char('n') | crate::ui::KeyCode::Char('N') => {
                Action::Session(SessionAction::NewProject)
            }
            crate::ui::KeyCode::Char('d') | crate::ui::KeyCode::Char('D') => {
                if let Some(entry) = self.entries.get(self.selected) {
                    let path = entry.path.clone();
                    let mut recent = RecentProjects::load();
                    recent.remove(&path);
                    recent.save();
                    self.refresh();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let width = 56_u16.min(area.width.saturating_sub(4));
        let height = (self.entries.len() as u16 + 8).min(area.height.saturating_sub(4)).max(10);
        let rect = center_rect(area, width, height);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Projects ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        // Section header
        let header_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
        let header_area = RatatuiRect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1);
        Paragraph::new(Line::from(Span::styled("Recent Projects", header_style)))
            .render(header_area, buf);

        if self.entries.is_empty() {
            let empty_y = inner.y + 2;
            if empty_y < inner.y + inner.height {
                let empty_area = RatatuiRect::new(inner.x + 1, empty_y, inner.width.saturating_sub(2), 1);
                Paragraph::new(Line::from(Span::styled(
                    "No recent projects",
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                ))).render(empty_area, buf);
            }
        }

        // Project list
        let max_visible = (inner.height.saturating_sub(4)) as usize;
        let scroll = if self.selected >= max_visible {
            self.selected - max_visible + 1
        } else {
            0
        };

        for (i, entry) in self.entries.iter().skip(scroll).take(max_visible).enumerate() {
            let y = inner.y + 2 + i as u16;
            if y >= inner.y + inner.height.saturating_sub(2) {
                break;
            }

            let is_selected = scroll + i == self.selected;
            let time_str = Self::format_time_ago(entry.last_opened);

            let name_max = inner.width.saturating_sub(time_str.len() as u16 + 6) as usize;
            let display_name: String = entry.name.chars().take(name_max).collect();

            let (name_style, time_style) = if is_selected {
                (
                    ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::CYAN).bold()),
                    ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::CYAN)),
                )
            } else {
                (
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE)),
                    ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                )
            };

            // Clear the line for selected item
            if is_selected {
                let line_area = RatatuiRect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1);
                for x in line_area.x..line_area.x + line_area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ').set_style(name_style);
                    }
                }
            }

            let prefix = if is_selected { " > " } else { "   " };
            let padding_len = name_max.saturating_sub(display_name.len());
            let padding: String = " ".repeat(padding_len);

            let line = Line::from(vec![
                Span::styled(prefix, name_style),
                Span::styled(&display_name, name_style),
                Span::styled(&padding, name_style),
                Span::styled(format!("  {}", time_str), time_style),
            ]);
            let line_area = RatatuiRect::new(inner.x, y, inner.width, 1);
            Paragraph::new(line).render(line_area, buf);
        }

        // Footer
        let footer_y = rect.y + rect.height.saturating_sub(2);
        if footer_y < area.y + area.height {
            let footer_area = RatatuiRect::new(inner.x + 1, footer_y, inner.width.saturating_sub(2), 1);
            Paragraph::new(Line::from(vec![
                Span::styled("[N]", ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold())),
                Span::styled("ew  ", ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))),
                Span::styled("[Enter]", ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold())),
                Span::styled(" Open  ", ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))),
                Span::styled("[D]", ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold())),
                Span::styled("elete  ", ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))),
                Span::styled("[Esc]", ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold())),
                Span::styled(" Close", ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))),
            ])).render(footer_area, buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
