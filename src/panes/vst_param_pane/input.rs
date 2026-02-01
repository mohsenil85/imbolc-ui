use crate::state::automation::AutomationTarget;
use crate::state::AppState;
use crate::ui::{Action, AutomationAction, InputEvent, KeyCode, VstParamAction};

use super::VstParamPane;

impl VstParamPane {
    pub(super) fn handle_action_impl(&mut self, action: &str, _event: &InputEvent, state: &AppState) -> Action {
        if self.search_active {
            return match action {
                "escape" | "cancel" => {
                    self.search_active = false;
                    self.search_text.clear();
                    self.rebuild_filter(state);
                    Action::None
                }
                _ => Action::None,
            };
        }

        let Some(instrument_id) = self.instrument_id else {
            return Action::None;
        };

        match action {
            "up" | "prev" => {
                if self.selected_param > 0 {
                    self.selected_param -= 1;
                    // Adjust scroll
                    if self.selected_param < self.scroll_offset {
                        self.scroll_offset = self.selected_param;
                    }
                }
                Action::None
            }
            "down" | "next" => {
                if !self.filtered_indices.is_empty() && self.selected_param + 1 < self.filtered_indices.len() {
                    self.selected_param += 1;
                }
                Action::None
            }
            "left" | "adjust_down" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::VstParam(VstParamAction::AdjustParam(instrument_id, idx, -0.01));
                    }
                }
                Action::None
            }
            "right" | "adjust_up" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::VstParam(VstParamAction::AdjustParam(instrument_id, idx, 0.01));
                    }
                }
                Action::None
            }
            "coarse_left" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::VstParam(VstParamAction::AdjustParam(instrument_id, idx, -0.1));
                    }
                }
                Action::None
            }
            "coarse_right" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::VstParam(VstParamAction::AdjustParam(instrument_id, idx, 0.1));
                    }
                }
                Action::None
            }
            "reset" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::VstParam(VstParamAction::ResetParam(instrument_id, idx));
                    }
                }
                Action::None
            }
            "automate" => {
                if let Some(&param_idx) = self.filtered_indices.get(self.selected_param) {
                    let idx = self.get_param_index(param_idx, state);
                    if let Some(idx) = idx {
                        return Action::Automation(AutomationAction::AddLane(
                            AutomationTarget::VstParam(instrument_id, idx),
                        ));
                    }
                }
                Action::None
            }
            "discover" => {
                Action::VstParam(VstParamAction::DiscoverParams(instrument_id))
            }
            "search" => {
                self.search_active = true;
                self.search_text.clear();
                Action::None
            }
            "goto_top" => {
                self.selected_param = 0;
                self.scroll_offset = 0;
                Action::None
            }
            "goto_bottom" => {
                if !self.filtered_indices.is_empty() {
                    self.selected_param = self.filtered_indices.len() - 1;
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    pub(super) fn handle_raw_input_impl(&mut self, event: &InputEvent, state: &AppState) -> Action {
        if self.search_active {
            match event.key {
                KeyCode::Char(c) => {
                    self.search_text.push(c);
                    self.rebuild_filter(state);
                    self.selected_param = 0;
                    self.scroll_offset = 0;
                    return Action::None;
                }
                KeyCode::Backspace => {
                    self.search_text.pop();
                    self.rebuild_filter(state);
                    self.selected_param = 0;
                    self.scroll_offset = 0;
                    return Action::None;
                }
                KeyCode::Escape | KeyCode::Enter => {
                    self.search_active = false;
                    return Action::None;
                }
                _ => {}
            }
        }
        Action::None
    }

    /// Get the VST parameter index for a filtered param index
    fn get_param_index(&self, filtered_idx: usize, state: &AppState) -> Option<u32> {
        let inst = self.instrument_id
            .and_then(|id| state.instruments.instrument(id))?;
        if let crate::state::SourceType::Vst(plugin_id) = inst.source {
            let plugin = state.session.vst_plugins.get(plugin_id)?;
            plugin.params.get(filtered_idx).map(|p| p.index)
        } else {
            None
        }
    }
}
