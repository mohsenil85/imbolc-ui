use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, CustomSynthDefRegistry, SourceType};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{Action, Color, FileSelectAction, InputEvent, InstrumentAction, Keymap, NavAction, Pane, SessionAction, Style};

/// Options available in the Add Instrument menu
#[derive(Debug, Clone)]
pub enum AddOption {
    Source(SourceType),
    Separator(&'static str),
    ImportCustom,
}

pub struct AddPane {
    keymap: Keymap,
    selected: usize,
    /// Cached options list - rebuilt on each render_with_registry call
    cached_options: Vec<AddOption>,
}

impl AddPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            selected: 0,
            cached_options: Self::build_options_static(),
        }
    }

    /// Build options without custom synthdefs (used for initial state)
    fn build_options_static() -> Vec<AddOption> {
        let mut options = Vec::new();

        // Built-in types
        for source in SourceType::all() {
            options.push(AddOption::Source(source));
        }

        // Custom section
        options.push(AddOption::Separator("── Custom ──"));
        options.push(AddOption::ImportCustom);

        options
    }

    /// Build options with custom synthdefs from registry
    fn build_options(&self, registry: &CustomSynthDefRegistry) -> Vec<AddOption> {
        let mut options = Vec::new();

        // Built-in types
        for source in SourceType::all() {
            options.push(AddOption::Source(source));
        }

        // Custom section
        options.push(AddOption::Separator("── Custom ──"));

        // Custom synthdefs
        for synthdef in &registry.synthdefs {
            options.push(AddOption::Source(SourceType::Custom(synthdef.id)));
        }

        // Import option
        options.push(AddOption::ImportCustom);

        options
    }

    /// Update cached options from registry
    pub fn update_options(&mut self, registry: &CustomSynthDefRegistry) {
        self.cached_options = self.build_options(registry);
        // Clamp selection
        if self.selected >= self.cached_options.len() {
            self.selected = self.cached_options.len().saturating_sub(1);
        }
    }

    /// Move to next selectable item
    fn select_next(&mut self) {
        let len = self.cached_options.len();
        if len == 0 {
            return;
        }

        let mut next = (self.selected + 1) % len;
        // Skip separators
        while matches!(self.cached_options.get(next), Some(AddOption::Separator(_))) {
            next = (next + 1) % len;
        }
        self.selected = next;
    }

    /// Move to previous selectable item
    fn select_prev(&mut self) {
        let len = self.cached_options.len();
        if len == 0 {
            return;
        }

        let mut prev = if self.selected == 0 {
            len - 1
        } else {
            self.selected - 1
        };
        // Skip separators
        while matches!(self.cached_options.get(prev), Some(AddOption::Separator(_))) {
            prev = if prev == 0 { len - 1 } else { prev - 1 };
        }
        self.selected = prev;
    }

    /// Render with registry for custom synthdef names (ratatui buffer path)
    fn render_buf_with_registry(&self, area: RatatuiRect, buf: &mut Buffer, registry: &CustomSynthDefRegistry) {
        let rect = center_rect(area, 97, 29);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Add Instrument ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::LIME)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::LIME)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let content_x = inner.x + 1;
        let content_y = inner.y + 1;

        // Title
        Paragraph::new(Line::from(Span::styled(
            "Select source type:",
            ratatui::style::Style::from(Style::new().fg(Color::LIME).bold()),
        ))).render(RatatuiRect::new(content_x, content_y, inner.width.saturating_sub(2), 1), buf);

        let list_y = content_y + 2;
        let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));

        for (i, option) in self.cached_options.iter().enumerate() {
            let y = list_y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let is_selected = i == self.selected;

            match option {
                AddOption::Separator(label) => {
                    Paragraph::new(Line::from(Span::styled(
                        *label,
                        ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                    ))).render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
                }
                AddOption::Source(source) => {
                    // Indicator
                    if is_selected {
                        if let Some(cell) = buf.cell_mut((content_x, y)) {
                            cell.set_char('>').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                            );
                        }
                    }

                    let color = match source {
                        SourceType::AudioIn => Color::AUDIO_IN_COLOR,
                        SourceType::BusIn => Color::BUS_IN_COLOR,
                        SourceType::Sample => Color::SAMPLE_COLOR,
                        SourceType::Custom(_) => Color::CUSTOM_COLOR,
                        _ => Color::OSC_COLOR,
                    };

                    let short = format!("{:12}", source.short_name_with_registry(registry));
                    let name = source.display_name(registry);

                    let short_style = if is_selected {
                        ratatui::style::Style::from(Style::new().fg(color).bg(Color::SELECTION_BG))
                    } else {
                        ratatui::style::Style::from(Style::new().fg(color))
                    };
                    let name_style = if is_selected {
                        ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG))
                    } else {
                        ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY))
                    };

                    let line = Line::from(vec![
                        Span::styled(short, short_style),
                        Span::styled(format!("  {}", name), name_style),
                    ]);
                    Paragraph::new(line).render(
                        RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf,
                    );

                    // Fill rest of line with selection bg
                    if is_selected {
                        let fill_start = content_x + 2 + 14 + name.len() as u16;
                        let fill_end = inner.x + inner.width;
                        for x in fill_start..fill_end {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(' ').set_style(sel_bg);
                            }
                        }
                    }
                }
                AddOption::ImportCustom => {
                    if is_selected {
                        if let Some(cell) = buf.cell_mut((content_x, y)) {
                            cell.set_char('>').set_style(
                                ratatui::style::Style::from(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold()),
                            );
                        }
                    }

                    let text_style = if is_selected {
                        ratatui::style::Style::from(Style::new().fg(Color::PURPLE).bg(Color::SELECTION_BG))
                    } else {
                        ratatui::style::Style::from(Style::new().fg(Color::PURPLE))
                    };
                    Paragraph::new(Line::from(Span::styled(
                        "+ Import Custom SynthDef...",
                        text_style,
                    ))).render(RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1), buf);

                    if is_selected {
                        let fill_start = content_x + 2 + 27;
                        let fill_end = inner.x + inner.width;
                        for x in fill_start..fill_end {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(' ').set_style(sel_bg);
                            }
                        }
                    }
                }
            }
        }

        // Help text
        let help_y = rect.y + rect.height - 2;
        if help_y < area.y + area.height {
            Paragraph::new(Line::from(Span::styled(
                "Enter: add | Escape: cancel | Up/Down: navigate",
                ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
            ))).render(RatatuiRect::new(content_x, help_y, inner.width.saturating_sub(2), 1), buf);
        }
    }

}

impl Default for AddPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for AddPane {
    fn id(&self) -> &'static str {
        "add"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "confirm" => {
                if let Some(option) = self.cached_options.get(self.selected) {
                    match option {
                        AddOption::Source(source) => Action::Instrument(InstrumentAction::Add(*source)),
                        AddOption::ImportCustom => {
                            Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::ImportCustomSynthDef))
                        }
                        AddOption::Separator(_) => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            "cancel" => Action::Nav(NavAction::SwitchPane("instrument")),
            "next" => {
                self.select_next();
                Action::None
            }
            "prev" => {
                self.select_prev();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_buf_with_registry(area, buf, &state.session.custom_synthdefs);
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn on_enter(&mut self, state: &AppState) {
        self.update_options(&state.session.custom_synthdefs);
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
