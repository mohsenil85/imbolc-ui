use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::state::{AppState, EffectType, VstPluginRegistry};
use crate::ui::layout_helpers::center_rect;
use crate::ui::{
    Action, Color, FileSelectAction, InputEvent, InstrumentAction, Keymap, MouseEvent,
    MouseEventKind, MouseButton, NavAction, Pane, SessionAction, Style,
};

/// Options available in the Add Effect menu
#[derive(Debug, Clone)]
enum AddEffectOption {
    Effect(EffectType),
    Separator(&'static str),
    ImportVst,
}

pub struct AddEffectPane {
    keymap: Keymap,
    selected: usize,
    cached_options: Vec<AddEffectOption>,
}

impl AddEffectPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            selected: 0,
            cached_options: Self::build_options_static(),
        }
    }

    fn build_options_static() -> Vec<AddEffectOption> {
        Self::build_effect_list(&[])
    }

    fn build_effect_list(vst_effects: &[(u32, EffectType)]) -> Vec<AddEffectOption> {
        let mut options = vec![
            AddEffectOption::Separator("── Dynamics ──"),
            AddEffectOption::Effect(EffectType::TapeComp),
            AddEffectOption::Effect(EffectType::SidechainComp),
            AddEffectOption::Effect(EffectType::Gate),
            AddEffectOption::Effect(EffectType::Limiter),
            AddEffectOption::Separator("── Modulation ──"),
            AddEffectOption::Effect(EffectType::Chorus),
            AddEffectOption::Effect(EffectType::Flanger),
            AddEffectOption::Effect(EffectType::Phaser),
            AddEffectOption::Effect(EffectType::Tremolo),
            AddEffectOption::Separator("── Distortion ──"),
            AddEffectOption::Effect(EffectType::Distortion),
            AddEffectOption::Effect(EffectType::Bitcrusher),
            AddEffectOption::Effect(EffectType::Wavefolder),
            AddEffectOption::Effect(EffectType::Saturator),
            AddEffectOption::Separator("── EQ ──"),
            AddEffectOption::Effect(EffectType::TiltEq),
            AddEffectOption::Separator("── Stereo ──"),
            AddEffectOption::Effect(EffectType::StereoWidener),
            AddEffectOption::Effect(EffectType::FreqShifter),
            AddEffectOption::Separator("── Delay / Reverb ──"),
            AddEffectOption::Effect(EffectType::Delay),
            AddEffectOption::Effect(EffectType::Reverb),
            AddEffectOption::Effect(EffectType::ConvolutionReverb),
            AddEffectOption::Separator("── Utility ──"),
            AddEffectOption::Effect(EffectType::PitchShifter),
            AddEffectOption::Separator("── Lo-fi ──"),
            AddEffectOption::Effect(EffectType::Vinyl),
            AddEffectOption::Effect(EffectType::Cabinet),
            AddEffectOption::Separator("── Granular ──"),
            AddEffectOption::Effect(EffectType::GranularDelay),
            AddEffectOption::Effect(EffectType::GranularFreeze),
        ];

        options.push(AddEffectOption::Separator("── VST ──"));

        for &(_, effect_type) in vst_effects {
            options.push(AddEffectOption::Effect(effect_type));
        }

        options.push(AddEffectOption::ImportVst);

        options
    }

    fn build_options(&self, vst_registry: &VstPluginRegistry) -> Vec<AddEffectOption> {
        let vst_effects: Vec<(u32, EffectType)> = vst_registry
            .effects()
            .map(|p| (p.id, EffectType::Vst(p.id)))
            .collect();
        Self::build_effect_list(&vst_effects)
    }

    fn update_options(&mut self, vst_registry: &VstPluginRegistry) {
        self.cached_options = self.build_options(vst_registry);
        if self.selected >= self.cached_options.len() {
            self.selected = self.cached_options.len().saturating_sub(1);
        }
        // Ensure selection is not on a separator
        if matches!(self.cached_options.get(self.selected), Some(AddEffectOption::Separator(_))) {
            self.select_next();
        }
    }

    fn select_next(&mut self) {
        let len = self.cached_options.len();
        if len == 0 {
            return;
        }
        let mut next = (self.selected + 1) % len;
        while matches!(self.cached_options.get(next), Some(AddEffectOption::Separator(_))) {
            next = (next + 1) % len;
        }
        self.selected = next;
    }

    fn select_prev(&mut self) {
        let len = self.cached_options.len();
        if len == 0 {
            return;
        }
        let mut prev = if self.selected == 0 { len - 1 } else { self.selected - 1 };
        while matches!(self.cached_options.get(prev), Some(AddEffectOption::Separator(_))) {
            prev = if prev == 0 { len - 1 } else { prev - 1 };
        }
        self.selected = prev;
    }
}

impl Default for AddEffectPane {
    fn default() -> Self {
        Self::new(Keymap::new())
    }
}

impl Pane for AddEffectPane {
    fn id(&self) -> &'static str {
        "add_effect"
    }

    fn handle_action(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        match action {
            "confirm" => {
                if let Some(option) = self.cached_options.get(self.selected) {
                    match option {
                        AddEffectOption::Effect(effect_type) => {
                            if let Some(inst) = state.instruments.selected_instrument() {
                                Action::Instrument(InstrumentAction::AddEffect(inst.id, *effect_type))
                            } else {
                                Action::None
                            }
                        }
                        AddEffectOption::ImportVst => {
                            Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::ImportVstEffect))
                        }
                        AddEffectOption::Separator(_) => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            "cancel" => Action::Nav(NavAction::PopPane),
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

    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action {
        let rect = center_rect(area, 40, 20);
        let inner_y = rect.y + 2;
        let content_y = inner_y + 1;
        let list_y = content_y + 2;

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let row = event.row;
                if row >= list_y {
                    let idx = (row - list_y) as usize;
                    if idx < self.cached_options.len() {
                        if matches!(self.cached_options.get(idx), Some(AddEffectOption::Separator(_))) {
                            return Action::None;
                        }
                        self.selected = idx;
                        // Confirm on click
                        match &self.cached_options[idx] {
                            AddEffectOption::Effect(effect_type) => {
                                if let Some(inst) = state.instruments.selected_instrument() {
                                    return Action::Instrument(InstrumentAction::AddEffect(inst.id, *effect_type));
                                }
                            }
                            AddEffectOption::ImportVst => {
                                return Action::Session(SessionAction::OpenFileBrowser(FileSelectAction::ImportVstEffect));
                            }
                            AddEffectOption::Separator(_) => {}
                        }
                    }
                }
                Action::None
            }
            MouseEventKind::ScrollUp => {
                self.select_prev();
                Action::None
            }
            MouseEventKind::ScrollDown => {
                self.select_next();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        let vst_registry = &state.session.vst_plugins;
        let rect = center_rect(area, 40, 20);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Add Effect ")
            .border_style(ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR)))
            .title_style(ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR)));
        let inner = block.inner(rect);
        block.render(rect, buf);

        let content_x = inner.x + 1;
        let content_y = inner.y + 1;

        // Title
        Paragraph::new(Line::from(Span::styled(
            "Select effect type:",
            ratatui::style::Style::from(Style::new().fg(Color::FX_COLOR).bold()),
        )))
        .render(RatatuiRect::new(content_x, content_y, inner.width.saturating_sub(2), 1), buf);

        let list_y = content_y + 2;
        let sel_bg = ratatui::style::Style::from(Style::new().bg(Color::SELECTION_BG));

        for (i, option) in self.cached_options.iter().enumerate() {
            let y = list_y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let is_selected = i == self.selected;

            match option {
                AddEffectOption::Separator(label) => {
                    Paragraph::new(Line::from(Span::styled(
                        *label,
                        ratatui::style::Style::from(Style::new().fg(Color::DARK_GRAY)),
                    )))
                    .render(RatatuiRect::new(content_x, y, inner.width.saturating_sub(2), 1), buf);
                }
                AddEffectOption::Effect(effect_type) => {
                    if is_selected {
                        if let Some(cell) = buf.cell_mut((content_x, y)) {
                            cell.set_char('>').set_style(
                                ratatui::style::Style::from(
                                    Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold(),
                                ),
                            );
                        }
                    }

                    let color = if effect_type.is_vst() { Color::VST_COLOR } else { Color::FX_COLOR };
                    let name = effect_type.display_name(vst_registry);

                    let name_style = if is_selected {
                        ratatui::style::Style::from(Style::new().fg(color).bg(Color::SELECTION_BG))
                    } else {
                        ratatui::style::Style::from(Style::new().fg(color))
                    };

                    Paragraph::new(Line::from(Span::styled(name.clone(), name_style))).render(
                        RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1),
                        buf,
                    );

                    if is_selected {
                        let fill_start = content_x + 2 + name.len() as u16;
                        let fill_end = inner.x + inner.width;
                        for x in fill_start..fill_end {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(' ').set_style(sel_bg);
                            }
                        }
                    }
                }
                AddEffectOption::ImportVst => {
                    if is_selected {
                        if let Some(cell) = buf.cell_mut((content_x, y)) {
                            cell.set_char('>').set_style(
                                ratatui::style::Style::from(
                                    Style::new().fg(Color::WHITE).bg(Color::SELECTION_BG).bold(),
                                ),
                            );
                        }
                    }

                    let text_style = if is_selected {
                        ratatui::style::Style::from(Style::new().fg(Color::VST_COLOR).bg(Color::SELECTION_BG))
                    } else {
                        ratatui::style::Style::from(Style::new().fg(Color::VST_COLOR))
                    };
                    let label = "+ Import VST Effect...";
                    Paragraph::new(Line::from(Span::styled(label, text_style))).render(
                        RatatuiRect::new(content_x + 2, y, inner.width.saturating_sub(4), 1),
                        buf,
                    );

                    if is_selected {
                        let fill_start = content_x + 2 + label.len() as u16;
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
            )))
            .render(RatatuiRect::new(content_x, help_y, inner.width.saturating_sub(2), 1), buf);
        }
    }

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn on_enter(&mut self, state: &AppState) {
        self.update_options(&state.session.vst_plugins);
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
