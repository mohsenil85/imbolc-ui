mod input;
mod rendering;

use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect as RatatuiRect;

use crate::state::{AppState, InstrumentId};
use crate::ui::{Action, InputEvent, Keymap, Pane};

pub struct VstParamPane {
    keymap: Keymap,
    instrument_id: Option<InstrumentId>,
    selected_param: usize,
    scroll_offset: usize,
    search_text: String,
    search_active: bool,
    filtered_indices: Vec<usize>,
}

impl VstParamPane {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            instrument_id: None,
            selected_param: 0,
            scroll_offset: 0,
            search_text: String::new(),
            search_active: false,
            filtered_indices: Vec::new(),
        }
    }

    /// Rebuild filtered indices based on search text
    fn rebuild_filter(&mut self, state: &AppState) {
        let Some(inst) = self.instrument_id
            .and_then(|id| state.instruments.instrument(id)) else {
            self.filtered_indices.clear();
            return;
        };

        let plugin_params = if let crate::state::SourceType::Vst(plugin_id) = inst.source {
            state.session.vst_plugins.get(plugin_id)
                .map(|p| &p.params)
        } else {
            None
        };

        let Some(params) = plugin_params else {
            self.filtered_indices.clear();
            return;
        };

        if self.search_text.is_empty() {
            self.filtered_indices = (0..params.len()).collect();
        } else {
            let query = self.search_text.to_lowercase();
            self.filtered_indices = params.iter()
                .enumerate()
                .filter(|(_, p)| p.name.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Sync state from current selection
    fn sync_from_state(&mut self, state: &AppState) {
        let new_id = state.instruments.selected_instrument().map(|i| i.id);
        if new_id != self.instrument_id {
            self.instrument_id = new_id;
            self.selected_param = 0;
            self.scroll_offset = 0;
            self.search_text.clear();
            self.search_active = false;
        }
        self.rebuild_filter(state);
    }
}

impl Pane for VstParamPane {
    fn id(&self) -> &'static str {
        "vst_params"
    }

    fn handle_action(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action {
        self.sync_from_state(state);
        self.handle_action_impl(action, event, state)
    }

    fn handle_raw_input(&mut self, event: &InputEvent, state: &AppState) -> Action {
        self.sync_from_state(state);
        self.handle_raw_input_impl(event, state)
    }

    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState) {
        self.render_impl(area, buf, state);
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
    use crate::ui::Keymap;

    #[test]
    fn vst_param_pane_id() {
        let pane = VstParamPane::new(Keymap::new());
        assert_eq!(pane.id(), "vst_params");
    }
}
