use std::any::Any;

use crate::state::{AppState, CustomSynthDefRegistry, OscType};
use crate::ui::{Action, Color, FileSelectAction, Graphics, InputEvent, InstrumentAction, Keymap, NavAction, Pane, Rect, SessionAction, Style};

/// Options available in the Add Instrument menu
#[derive(Debug, Clone)]
pub enum AddOption {
    OscType(OscType),
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
        for osc in OscType::all() {
            options.push(AddOption::OscType(osc));
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
        for osc in OscType::all() {
            options.push(AddOption::OscType(osc));
        }

        // Custom section
        options.push(AddOption::Separator("── Custom ──"));

        // Custom synthdefs
        for synthdef in &registry.synthdefs {
            options.push(AddOption::OscType(OscType::Custom(synthdef.id)));
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

    /// Render with registry for custom synthdef names
    pub fn render_with_registry(&self, g: &mut dyn Graphics, registry: &CustomSynthDefRegistry) {
        let (width, height) = g.size();
        let box_width = 97;
        let box_height = 29;
        let rect = Rect::centered(width, height, box_width, box_height);

        g.set_style(Style::new().fg(Color::LIME));
        g.draw_box(rect, Some(" Add Instrument "));

        let content_x = rect.x + 2;
        let content_y = rect.y + 2;

        g.set_style(Style::new().fg(Color::LIME).bold());
        g.put_str(content_x, content_y, "Select source type:");

        let list_y = content_y + 2;
        for (i, option) in self.cached_options.iter().enumerate() {
            let y = list_y + i as u16;
            let is_selected = i == self.selected;

            match option {
                AddOption::Separator(label) => {
                    g.set_style(Style::new().fg(Color::DARK_GRAY));
                    g.put_str(content_x, y, label);
                }
                AddOption::OscType(osc) => {
                    if is_selected {
                        g.set_style(
                            Style::new()
                                .fg(Color::WHITE)
                                .bg(Color::SELECTION_BG)
                                .bold(),
                        );
                        g.put_str(content_x, y, ">");
                    } else {
                        g.set_style(Style::new().fg(Color::DARK_GRAY));
                        g.put_str(content_x, y, " ");
                    }

                    // Color based on type
                    let color = match osc {
                        OscType::AudioIn => Color::AUDIO_IN_COLOR,
                        OscType::BusIn => Color::BUS_IN_COLOR,
                        OscType::Sample => Color::SAMPLE_COLOR,
                        OscType::Custom(_) => Color::CUSTOM_COLOR,
                        _ => Color::OSC_COLOR,
                    };

                    if is_selected {
                        g.set_style(Style::new().fg(color).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(color));
                    }

                    let short = osc.short_name_with_registry(registry);
                    g.put_str(content_x + 2, y, &format!("{:12}", short));

                    if is_selected {
                        g.set_style(Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(Color::DARK_GRAY));
                    }

                    let name = osc.display_name(registry);
                    g.put_str(content_x + 15, y, &name);

                    // Fill rest of line if selected
                    if is_selected {
                        g.set_style(Style::new().bg(Color::SELECTION_BG));
                        let line_end = content_x + 15 + name.len() as u16;
                        for x in line_end..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                }
                AddOption::ImportCustom => {
                    if is_selected {
                        g.set_style(
                            Style::new()
                                .fg(Color::WHITE)
                                .bg(Color::SELECTION_BG)
                                .bold(),
                        );
                        g.put_str(content_x, y, ">");
                    } else {
                        g.set_style(Style::new().fg(Color::DARK_GRAY));
                        g.put_str(content_x, y, " ");
                    }

                    if is_selected {
                        g.set_style(Style::new().fg(Color::PURPLE).bg(Color::SELECTION_BG));
                    } else {
                        g.set_style(Style::new().fg(Color::PURPLE));
                    }
                    g.put_str(content_x + 2, y, "+ Import Custom SynthDef...");

                    if is_selected {
                        g.set_style(Style::new().bg(Color::SELECTION_BG));
                        let line_end = content_x + 2 + 27; // length of text
                        for x in line_end..(rect.x + rect.width - 2) {
                            g.put_char(x, y, ' ');
                        }
                    }
                }
            }
        }

        let help_y = rect.y + rect.height - 2;
        g.set_style(Style::new().fg(Color::DARK_GRAY));
        g.put_str(
            content_x,
            help_y,
            "Enter: add | Escape: cancel | Up/Down: navigate",
        );
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

    fn handle_input(&mut self, event: InputEvent, _state: &AppState) -> Action {
        match self.keymap.lookup(&event) {
            Some("confirm") => {
                if let Some(option) = self.cached_options.get(self.selected) {
                    match option {
                        AddOption::OscType(osc) => Action::Instrument(InstrumentAction::Add(*osc)),
                        AddOption::ImportCustom => {
                            Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::ImportCustomSynthDef))
                        }
                        AddOption::Separator(_) => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            Some("cancel") => Action::Nav(NavAction::SwitchPane("instrument")),
            Some("next") => {
                self.select_next();
                Action::None
            }
            Some("prev") => {
                self.select_prev();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, g: &mut dyn Graphics, state: &AppState) {
        self.render_with_registry(g, &state.session.custom_synthdefs);
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
