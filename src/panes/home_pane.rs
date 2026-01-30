use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::AppState;
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, InputEvent, Keymap, NavAction, Pane, Style};

/// Menu item for the home screen
struct MenuItem {
    label: &'static str,
    description: &'static str,
    pane_id: &'static str,
}

pub struct HomePane {
    keymap: Keymap,
    selected: usize,
    items: Vec<MenuItem>,
}

impl HomePane {
    pub fn new(keymap: Keymap) -> Self {
        let items = vec![
            MenuItem {
                label: "Instruments",
                description: "Instrument list - add and edit synths",
                pane_id: "instrument",
            },
            MenuItem {
                label: "Mixer",
                description: "Mixing console - adjust levels and routing",
                pane_id: "mixer",
            },
            MenuItem {
                label: "Server",
                description: "Audio server - start/stop and manage SuperCollider",
                pane_id: "server",
            },
        ];

        Self {
            keymap,
            selected: 0,
            items,
        }
    }
}

impl Default for HomePane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for HomePane {
    fn id(&self) -> &'static str {
        "home"
    }

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("up") => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            Some("down") => {
                if self.selected < self.items.len() - 1 {
                    self.selected += 1;
                }
                Action::None
            }
            Some("select") => Action::Nav(NavAction::SwitchPane(self.items[self.selected].pane_id)),
            Some("quit") => Action::Quit,
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, _state: &AppState) {
        let rect = center_rect(area, 50, 12);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ILEX ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::MAGENTA)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::MAGENTA)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let item_colors = [Color::CYAN, Color::PURPLE, Color::GOLD];

        for (i, item) in self.items.iter().enumerate() {
            let y = inner.y + 1 + (i as u16 * 2);
            let is_selected = i == self.selected;
            let item_color = item_colors.get(i).copied().unwrap_or(Color::WHITE);

            let label_text = format!(" [{}] {} ", i + 1, item.label);

            let label_line = if is_selected {
                Line::from(Span::styled(
                    label_text,
                    ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                ))
            } else {
                Line::from(Span::styled(
                    label_text,
                    ratatui::style::Style::from(Style::new().fg(item_color)),
                ))
            };

            let desc_style = if is_selected {
                ratatui::style::Style::from(Style::new().fg(Color::SKY_BLUE))
            } else {
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
            };
            let desc_line = Line::from(Span::styled(format!("  {}", item.description), desc_style));

            if y < inner.y + inner.height {
                let label_area = RatatuiRect::new(inner.x + 2, y, inner.width.saturating_sub(2), 1);
                Paragraph::new(label_line).render(label_area, buf);
            }
            if y + 1 < inner.y + inner.height {
                let desc_area = RatatuiRect::new(inner.x + 2, y + 1, inner.width.saturating_sub(2), 1);
                Paragraph::new(desc_line).render(desc_area, buf);
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        if help_y < area.y + area.height {
            let help_area = RatatuiRect::new(inner.x + 2, help_y, inner.width.saturating_sub(2), 1);
            let help = Paragraph::new(Line::from(Span::styled(
                "[1-3] Jump  [Enter] Select  [q] Quit",
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            )));
            help.render(help_area, buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
