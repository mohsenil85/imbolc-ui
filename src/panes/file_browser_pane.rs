use std::any::Any;
use std::fs;
use std::path::PathBuf;

use crate::state::AppState;
use crate::ui::{
    Action, Color, FileSelectAction, Graphics, InputEvent, KeyCode, Keymap, Pane, Rect, Style,
};

struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

pub struct FileBrowserPane {
    keymap: Keymap,
    current_dir: PathBuf,
    entries: Vec<DirEntry>,
    selected: usize,
    filter_extension: Option<String>,
    on_select_action: FileSelectAction,
    scroll_offset: usize,
}

impl FileBrowserPane {
    pub fn new() -> Self {
        let start_dir = std::env::current_dir().unwrap_or_else(|_| {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        });
        let mut pane = Self {
            keymap: Keymap::new()
                .bind_key(KeyCode::Enter, "select", "Select file/enter directory")
                .bind_key(KeyCode::Escape, "cancel", "Cancel and return")
                .bind_key(KeyCode::Backspace, "parent", "Go to parent directory")
                .bind('h', "parent", "Go to parent directory")
                .bind_key(KeyCode::Down, "next", "Next entry")
                .bind('j', "next", "Next entry")
                .bind_key(KeyCode::Up, "prev", "Previous entry")
                .bind('k', "prev", "Previous entry")
                .bind('~', "home", "Go to home directory")
                .bind_key(KeyCode::Home, "goto_top", "Go to top")
                .bind_key(KeyCode::End, "goto_bottom", "Go to bottom"),
            current_dir: start_dir,
            entries: Vec::new(),
            selected: 0,
            filter_extension: Some("scd".to_string()),
            on_select_action: FileSelectAction::ImportCustomSynthDef,
            scroll_offset: 0,
        };
        pane.refresh_entries();
        pane
    }

    /// Open for a specific action with optional start directory
    pub fn open_for(&mut self, action: FileSelectAction, start_dir: Option<PathBuf>) {
        self.on_select_action = action.clone();
        self.filter_extension = match action {
            FileSelectAction::ImportCustomSynthDef => Some("scd".to_string()),
        };
        self.current_dir = start_dir.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
            })
        });
        self.selected = 0;
        self.scroll_offset = 0;
        self.refresh_entries();
    }

    fn refresh_entries(&mut self) {
        self.entries.clear();

        if let Ok(read_dir) = fs::read_dir(&self.current_dir) {
            let mut dirs: Vec<DirEntry> = Vec::new();
            let mut files: Vec<DirEntry> = Vec::new();

            for entry in read_dir.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files
                if name.starts_with('.') {
                    continue;
                }

                let is_dir = path.is_dir();

                // Filter files by extension if set
                if !is_dir {
                    if let Some(ref ext) = self.filter_extension {
                        if path.extension().map_or(true, |e| e != ext.as_str()) {
                            continue;
                        }
                    }
                }

                let entry = DirEntry { name, path, is_dir };
                if is_dir {
                    dirs.push(entry);
                } else {
                    files.push(entry);
                }
            }

            // Sort alphabetically
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            self.entries.extend(dirs);
            self.entries.extend(files);
        }

        // Clamp selection
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
    }
}

impl Default for FileBrowserPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for FileBrowserPane {
    fn id(&self) -> &'static str {
        "file_browser"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("select") => {
                if let Some(entry) = self.entries.get(self.selected) {
                    if entry.is_dir {
                        self.current_dir = entry.path.clone();
                        self.selected = 0;
                        self.scroll_offset = 0;
                        self.refresh_entries();
                        Action::None
                    } else {
                        // File selected
                        match self.on_select_action {
                            FileSelectAction::ImportCustomSynthDef => {
                                Action::ImportCustomSynthDef(entry.path.clone())
                            }
                        }
                    }
                } else {
                    Action::None
                }
            }
            Some("cancel") => Action::SwitchPane("add"),
            Some("parent") => {
                if let Some(parent) = self.current_dir.parent() {
                    self.current_dir = parent.to_path_buf();
                    self.selected = 0;
                    self.scroll_offset = 0;
                    self.refresh_entries();
                }
                Action::None
            }
            Some("home") => {
                if let Some(home) = dirs::home_dir() {
                    self.current_dir = home;
                    self.selected = 0;
                    self.scroll_offset = 0;
                    self.refresh_entries();
                }
                Action::None
            }
            Some("next") => {
                if !self.entries.is_empty() {
                    self.selected = (self.selected + 1).min(self.entries.len() - 1);
                }
                Action::None
            }
            Some("prev") => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            Some("goto_top") => {
                self.selected = 0;
                self.scroll_offset = 0;
                Action::None
            }
            Some("goto_bottom") => {
                if !self.entries.is_empty() {
                    self.selected = self.entries.len() - 1;
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, _state: &AppState) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::PURPLE));
        g.draw_box(rect, Some(" Import Custom SynthDef "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        // Current path
        g.set_style(Style::new().fg(Color::CYAN).bold());
        let path_str = self.current_dir.to_string_lossy();
        let max_path_width = (rect.width - 4) as usize;
        let display_path = if path_str.len() > max_path_width {
            format!("...{}", &path_str[path_str.len() - max_path_width + 3..])
        } else {
            path_str.to_string()
        };
        g.put_str(content_x, content_y, &display_path);

        // File list
        let list_y = content_y + 2;
        let visible_height = (rect.height - 7) as usize; // Account for padding and help

        // Clone to avoid borrow issues
        let entries = &self.entries;
        let selected = self.selected;
        let scroll_offset = self.scroll_offset;

        // Calculate scroll offset for visibility
        let mut eff_scroll = scroll_offset;
        if selected < eff_scroll {
            eff_scroll = selected;
        } else if selected >= eff_scroll + visible_height {
            eff_scroll = selected - visible_height + 1;
        }

        if entries.is_empty() {
            g.set_style(Style::new().fg(Color::DARK_GRAY));
            g.put_str(content_x, list_y, "(no .scd files found)");
        } else {
            for (i, entry) in entries
                .iter()
                .skip(eff_scroll)
                .take(visible_height)
                .enumerate()
            {
                let y = list_y + i as u16;
                let is_selected = eff_scroll + i == selected;

                if is_selected {
                    g.set_style(
                        Style::new()
                            .fg(Color::WHITE)
                            .bg(Color::SELECTION_BG)
                            .bold(),
                    );
                    // Fill selection background
                    for x in content_x..(rect.x + rect.width - 2) {
                        g.put_char(x, y, ' ');
                    }
                    g.put_str(content_x, y, ">");
                } else {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, y, " ");
                }

                // Icon and name
                let (icon, color) = if entry.is_dir {
                    ("/", Color::CYAN)
                } else {
                    (" ", Color::CUSTOM_COLOR)
                };

                if is_selected {
                    g.set_style(Style::new().fg(color).bg(Color::SELECTION_BG));
                } else {
                    g.set_style(Style::new().fg(color));
                }
                g.put_str(content_x + 2, y, icon);

                let max_name_width = (rect.width - 8) as usize;
                let display_name = if entry.name.len() > max_name_width {
                    format!("{}...", &entry.name[..max_name_width - 3])
                } else {
                    entry.name.clone()
                };

                if is_selected {
                    g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                } else {
                    g.set_style(Style::new().fg(if entry.is_dir {
                        Color::CYAN
                    } else {
                        Color::WHITE
                    }));
                }
                g.put_str(content_x + 4, y, &display_name);
            }

            // Scroll indicators
            if eff_scroll > 0 {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(rect.x + rect.width - 4, list_y, "...");
            }
            if eff_scroll + visible_height < entries.len() {
                g.set_style(Style::new().fg(Color::DARK_GRAY));
                g.put_str(
                    rect.x + rect.width - 4,
                    list_y + visible_height as u16 - 1,
                    "...",
                );
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            content_x,
            help_y,
            "Enter: select | Backspace: parent | ~: home | Esc: cancel",
        );
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn on_enter(&mut self, _state: &AppState) {
        self.refresh_entries();
    }
}
