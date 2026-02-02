use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, KeyCode, Keymap, NavAction, Pane, Style};

pub struct CommandPalettePane {
    keymap: Keymap,
    /// (action, description, keybinding display)
    commands: Vec<(String, String, String)>,
    input: String,
    cursor: usize,
    /// Indices into `commands` matching current filter
    filtered: Vec<usize>,
    /// Index within `filtered`
    selected: usize,
    scroll: usize,
    /// The manually-typed prefix (separate from `input` which changes during tab cycling)
    filter_base: String,
    pending_command: Option<String>,
}

impl CommandPalettePane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            commands: Vec::new(),
            input: String::new(),
            cursor: 0,
            filtered: Vec::new(),
            selected: 0,
            scroll: 0,
            filter_base: String::new(),
            pending_command: None,
        }
    }

    /// Called before push to populate the palette with available commands.
    pub fn open(&mut self, commands: Vec<(&'static str, &'static str, String)>) {
        self.commands = commands
            .into_iter()
            .map(|(a, d, k)| (a.to_string(), d.to_string(), k))
            .collect();
        self.input.clear();
        self.cursor = 0;
        self.filter_base.clear();
        self.pending_command = None;
        self.selected = 0;
        self.scroll = 0;
        self.update_filter();
    }

    /// Called by main.rs after pop to get the confirmed command.
    pub fn take_command(&mut self) -> Option<String> {
        self.pending_command.take()
    }

    fn update_filter(&mut self) {
        let query = self.filter_base.to_lowercase();
        self.filtered = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, (action, desc, _))| {
                if query.is_empty() {
                    return true;
                }
                action.to_lowercase().contains(&query)
                    || desc.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
        self.scroll = 0;
    }

    fn tab_complete(&mut self) {
        if self.filtered.is_empty() {
            return;
        }

        // Find longest common prefix of all filtered action names
        let first_action = &self.commands[self.filtered[0]].0;
        let mut lcp = first_action.clone();
        for &idx in &self.filtered[1..] {
            let action = &self.commands[idx].0;
            lcp = longest_common_prefix(&lcp, action);
            if lcp.is_empty() {
                break;
            }
        }

        if lcp.len() > self.input.len() && lcp.starts_with(&self.input) {
            // LCP extends beyond current input — fill in LCP
            self.input = lcp;
            self.cursor = self.input.len();
            self.filter_base = self.input.clone();
            self.update_filter();
        } else if self.filtered.len() == 1 {
            // Single match — fill in completely
            let action = self.commands[self.filtered[0]].0.clone();
            self.input = action;
            self.cursor = self.input.len();
            self.filter_base = self.input.clone();
            self.update_filter();
        } else if self.filtered.len() > 1 {
            // Already at LCP and multiple matches — cycle selected
            self.selected = (self.selected + 1) % self.filtered.len();
            self.ensure_visible();
            let idx = self.filtered[self.selected];
            self.input = self.commands[idx].0.clone();
            self.cursor = self.input.len();
            // Don't change filter_base — keep showing all matches
        }
    }

    fn ensure_visible(&mut self) {
        let max_visible = 10;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + max_visible {
            self.scroll = self.selected.saturating_sub(max_visible - 1);
        }
    }
}

fn longest_common_prefix(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca == cb)
        .map(|(c, _)| c)
        .collect()
}

impl Pane for CommandPalettePane {
    fn id(&self) -> &'static str {
        "command_palette"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "palette:confirm" => {
                if !self.filtered.is_empty() {
                    let idx = self.filtered[self.selected];
                    self.pending_command = Some(self.commands[idx].0.clone());
                } else if !self.input.is_empty() {
                    self.pending_command = Some(self.input.clone());
                }
                Action::Nav(NavAction::PopPane)
            }
            "palette:cancel" => {
                self.pending_command = None;
                Action::Nav(NavAction::PopPane)
            }
            _ => Action::None,
        }
    }

    fn handle_raw_input(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        match event.key {
            KeyCode::Char(ch) => {
                self.input.insert(self.cursor, ch);
                self.cursor += ch.len_utf8();
                self.filter_base = self.input.clone();
                self.update_filter();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = self.input[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input.drain(prev..self.cursor);
                    self.cursor = prev;
                    self.filter_base = self.input.clone();
                    self.update_filter();
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    let next = self.input[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor + i)
                        .unwrap_or(self.input.len());
                    self.input.drain(self.cursor..next);
                    self.filter_base = self.input.clone();
                    self.update_filter();
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor = self.input[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor = self.input[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor + i)
                        .unwrap_or(self.input.len());
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.input.len();
            }
            KeyCode::Tab => {
                self.tab_complete();
            }
            KeyCode::Up => {
                if !self.filtered.is_empty() {
                    if self.selected > 0 {
                        self.selected -= 1;
                    } else {
                        self.selected = self.filtered.len() - 1;
                    }
                    self.ensure_visible();
                    let idx = self.filtered[self.selected];
                    self.input = self.commands[idx].0.clone();
                    self.cursor = self.input.len();
                }
            }
            KeyCode::Down => {
                if !self.filtered.is_empty() {
                    self.selected = (self.selected + 1) % self.filtered.len();
                    self.ensure_visible();
                    let idx = self.filtered[self.selected];
                    self.input = self.commands[idx].0.clone();
                    self.cursor = self.input.len();
                }
            }
            _ => {}
        }
        Action::None
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let max_visible: usize = 10;
        let list_height = self.filtered.len().min(max_visible);
        // 1 for prompt + 1 for divider + list rows + 2 for border
        let total_height = (3 + list_height).max(5) as u16;
        let width = 60u16.min(area.width.saturating_sub(4));
        let rect = center_rect(area, width, total_height);

        // Clear background
        let bg_style = ratatui::style::Style::from(Style::new().bg(Color::new(20, 20, 30)));
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                    cell.set_symbol(" ");
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Command Palette ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::CYAN)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Prompt line: ": <input>"
        let prompt_y = inner.y;
        let prompt_area = RatatuiRect::new(inner.x, prompt_y, inner.width, 1);
        let before_cursor = &self.input[..self.cursor];
        let cursor_char = self.input[self.cursor..].chars().next().unwrap_or(' ');
        let after_cursor_start = self.cursor + cursor_char.len_utf8().min(self.input.len() - self.cursor);
        let after_cursor = if after_cursor_start <= self.input.len() {
            &self.input[after_cursor_start..]
        } else {
            ""
        };

        let prompt_style = ratatui::style::Style::from(Style::new().fg(Color::CYAN).bold());
        let input_style = ratatui::style::Style::from(Style::new().fg(Color::WHITE));
        let cursor_style = ratatui::style::Style::from(Style::new().fg(Color::BLACK).bg(Color::WHITE));

        let prompt_line = Line::from(vec![
            Span::styled(": ", prompt_style),
            Span::styled(before_cursor, input_style),
            Span::styled(cursor_char.to_string(), cursor_style),
            Span::styled(after_cursor, input_style),
        ]);
        Paragraph::new(prompt_line).render(prompt_area, buf);

        // Divider
        if inner.height > 1 {
            let div_y = inner.y + 1;
            let div_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
            let divider = "\u{2500}".repeat(inner.width as usize);
            let div_area = RatatuiRect::new(inner.x, div_y, inner.width, 1);
            Paragraph::new(Line::from(Span::styled(divider, div_style))).render(div_area, buf);
        }

        // Filtered list
        let list_start_y = inner.y + 2;
        let available_rows = (inner.height as usize).saturating_sub(2);

        if self.filtered.is_empty() {
            if available_rows > 0 {
                let no_match_style = ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY));
                let no_match_area = RatatuiRect::new(inner.x + 1, list_start_y, inner.width.saturating_sub(2), 1);
                Paragraph::new(Line::from(Span::styled("No matches", no_match_style)))
                    .render(no_match_area, buf);
            }
            return;
        }

        let visible_count = available_rows.min(self.filtered.len().saturating_sub(self.scroll));
        for row in 0..visible_count {
            let filter_idx = self.scroll + row;
            if filter_idx >= self.filtered.len() {
                break;
            }
            let cmd_idx = self.filtered[filter_idx];
            let (ref action, ref desc, ref key) = self.commands[cmd_idx];
            let y = list_start_y + row as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = filter_idx == self.selected;
            let row_area = RatatuiRect::new(inner.x, y, inner.width, 1);

            // Clear row with selection bg if selected
            if is_selected {
                let sel_style = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));
                for x in row_area.x..row_area.x + row_area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(sel_style);
                        cell.set_symbol(" ");
                    }
                }
            }

            let action_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold())
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::WHITE))
            };
            let desc_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::GRAY))
            };
            let key_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY).bg(Color::SELECTION_BG))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };

            // Layout: " action_name  description     keybinding "
            let w = inner.width as usize;
            let key_display = format!(" {} ", key);
            let key_len = key_display.len();
            let action_display = format!(" {}", action);
            let desc_display = format!("  {}", desc);

            // Truncate if needed
            let remaining = w.saturating_sub(key_len);
            let action_len = action_display.len().min(remaining);
            let desc_remaining = remaining.saturating_sub(action_len);
            let desc_len = desc_display.len().min(desc_remaining);
            let pad_len = w.saturating_sub(action_len + desc_len + key_len);

            let line = Line::from(vec![
                Span::styled(&action_display[..action_len], action_style),
                Span::styled(&desc_display[..desc_len], desc_style),
                Span::styled(" ".repeat(pad_len), desc_style),
                Span::styled(key_display, key_style),
            ]);
            Paragraph::new(line).render(row_area, buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
