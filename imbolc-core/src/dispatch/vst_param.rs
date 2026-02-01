use std::path::PathBuf;

use crate::audio::AudioHandle;
use crate::audio::commands::AudioCmd;
use crate::state::AppState;
use crate::action::{DispatchResult, VstParamAction};

/// Compute VST state file path for an instrument
fn vst_state_path(instrument_id: u32, plugin_name: &str) -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    let sanitized: String = plugin_name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    config_dir
        .join("imbolc")
        .join("vst_states")
        .join(format!("instrument_{}_{}.fxp", instrument_id, sanitized))
}

pub(super) fn dispatch_vst_param(
    action: &VstParamAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    match action {
        VstParamAction::SetParam(instrument_id, param_index, value) => {
            let value = value.clamp(0.0, 1.0);
            if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                // Update or insert the param value
                if let Some(entry) = instrument.vst_param_values.iter_mut().find(|(idx, _)| *idx == *param_index) {
                    entry.1 = value;
                } else {
                    instrument.vst_param_values.push((*param_index, value));
                }
            }
            if audio.is_running() {
                let _ = audio.send_cmd(AudioCmd::SetVstParam {
                    instrument_id: *instrument_id,
                    param_index: *param_index,
                    value,
                });
            }
            DispatchResult::none()
        }
        VstParamAction::AdjustParam(instrument_id, param_index, delta) => {
            let current = state.instruments.instrument(*instrument_id)
                .and_then(|inst| inst.vst_param_values.iter().find(|(idx, _)| *idx == *param_index))
                .map(|(_, v)| *v)
                .unwrap_or_else(|| {
                    // Look up default from VST plugin registry
                    if let Some(inst) = state.instruments.instrument(*instrument_id) {
                        if let crate::state::SourceType::Vst(plugin_id) = inst.source {
                            if let Some(plugin) = state.session.vst_plugins.get(plugin_id) {
                                if let Some(spec) = plugin.params.iter().find(|p| p.index == *param_index) {
                                    return spec.default;
                                }
                            }
                        }
                    }
                    0.5
                });
            let new_value = (current + delta).clamp(0.0, 1.0);
            // Re-dispatch as SetParam
            dispatch_vst_param(
                &VstParamAction::SetParam(*instrument_id, *param_index, new_value),
                state,
                audio,
            )
        }
        VstParamAction::ResetParam(instrument_id, param_index) => {
            // Look up default from VST plugin registry
            let default = state.instruments.instrument(*instrument_id)
                .and_then(|inst| {
                    if let crate::state::SourceType::Vst(plugin_id) = inst.source {
                        state.session.vst_plugins.get(plugin_id)
                            .and_then(|plugin| plugin.params.iter().find(|p| p.index == *param_index))
                            .map(|spec| spec.default)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.5);
            dispatch_vst_param(
                &VstParamAction::SetParam(*instrument_id, *param_index, default),
                state,
                audio,
            )
        }
        VstParamAction::DiscoverParams(instrument_id) => {
            if audio.is_running() {
                let _ = audio.send_cmd(AudioCmd::QueryVstParams {
                    instrument_id: *instrument_id,
                });
            }
            DispatchResult::none()
        }
        VstParamAction::SaveState(instrument_id) => {
            if let Some(instrument) = state.instruments.instrument(*instrument_id) {
                let plugin_name = if let crate::state::SourceType::Vst(plugin_id) = instrument.source {
                    state.session.vst_plugins.get(plugin_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    return DispatchResult::none();
                };
                let path = vst_state_path(*instrument_id, &plugin_name);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(instrument) = state.instruments.instrument_mut(*instrument_id) {
                    instrument.vst_state_path = Some(path.clone());
                }
                if audio.is_running() {
                    let _ = audio.send_cmd(AudioCmd::SaveVstState {
                        instrument_id: *instrument_id,
                        path,
                    });
                }
            }
            DispatchResult::none()
        }
    }
}
