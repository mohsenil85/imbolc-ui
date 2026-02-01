use crate::audio::AudioHandle;
use crate::scd_parser;
use crate::state::{AppState, CustomSynthDef, ParamSpec};
use crate::action::{DispatchResult, NavIntent, SessionAction};

use super::server::{compile_and_load_synthdef, config_synthdefs_dir};
use super::default_rack_path;

pub(super) fn dispatch_session(
    action: &SessionAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    let mut result = DispatchResult::none();

    match action {
        SessionAction::Save => {
            let path = default_rack_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Sync piano roll time_signature from session
            state.session.piano_roll.time_signature = state.session.time_signature;
            if let Err(e) = crate::state::persistence::save_project(&path, &state.session, &state.instruments) {
                eprintln!("Failed to save: {}", e);
            }
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("default")
                .to_string();
            result.project_name = Some(name);
        }
        SessionAction::Load => {
            let path = default_rack_path();
            if path.exists() {
                match crate::state::persistence::load_project(&path) {
                    Ok((loaded_session, loaded_instruments)) => {
                        state.session = loaded_session;
                        state.instruments = loaded_instruments;
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("default")
                            .to_string();
                        result.project_name = Some(name);
                        if state.instruments.instruments.is_empty() {
                            result.nav.push(NavIntent::SwitchTo("add"));
                        }
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.session = true;
                        result.audio_dirty.piano_roll = true;
                        result.audio_dirty.automation = true;
                        result.audio_dirty.routing = true;
                        result.audio_dirty.mixer_params = true;
                    }
                    Err(e) => {
                        eprintln!("Failed to load: {}", e);
                    }
                }
            }
        }
        SessionAction::UpdateSession(ref settings) => {
            state.session.apply_musical_settings(settings);
            state.session.piano_roll.time_signature = state.session.time_signature;
            state.session.piano_roll.bpm = state.session.bpm as f32;
            result.push_nav(NavIntent::PopOrSwitchTo("instrument"));
            result.audio_dirty.session = true;
            result.audio_dirty.piano_roll = true;
        }
        SessionAction::UpdateSessionLive(ref settings) => {
            state.session.apply_musical_settings(settings);
            state.session.piano_roll.time_signature = state.session.time_signature;
            state.session.piano_roll.bpm = state.session.bpm as f32;
            result.audio_dirty.session = true;
            result.audio_dirty.piano_roll = true;
        }
        SessionAction::OpenFileBrowser(ref file_action) => {
            result.push_nav(NavIntent::OpenFileBrowser(file_action.clone()));
        }
        SessionAction::ImportCustomSynthDef(ref path) => {
            // Read and parse the .scd file
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match scd_parser::parse_scd_file(&content) {
                        Ok(parsed) => {
                            // Create params with inferred ranges
                            let params: Vec<ParamSpec> = parsed
                                .params
                                .iter()
                                .map(|(name, default)| {
                                    let (min, max) =
                                        scd_parser::infer_param_range(name, *default);
                                    ParamSpec {
                                        name: name.clone(),
                                        default: *default,
                                        min,
                                        max,
                                    }
                                })
                                .collect();

                            // Create the custom synthdef entry
                            let synthdef_name = parsed.name.clone();
                            let custom = CustomSynthDef {
                                id: 0, // Will be set by registry.add()
                                name: parsed.name.clone(),
                                synthdef_name: synthdef_name.clone(),
                                source_path: path.clone(),
                                params,
                            };

                            // Register it
                            let _id = state.session.custom_synthdefs.add(custom);
                            result.audio_dirty.session = true;

                            // Copy the .scd file to the config synthdefs directory
                            let config_dir = config_synthdefs_dir();
                            let _ = std::fs::create_dir_all(&config_dir);

                            // Copy .scd file
                            if let Some(filename) = path.file_name() {
                                let dest = config_dir.join(filename);
                                let _ = std::fs::copy(path, &dest);
                            }

                            // Compile and load the synthdef
                            match compile_and_load_synthdef(path, &config_dir, &synthdef_name, audio) {
                                Ok(_) => {
                                    result.push_status(audio.status(), &format!("Loaded custom synthdef: {}", synthdef_name));
                                }
                                Err(e) => {
                                    eprintln!("Failed to compile/load synthdef: {}", e);
                                    result.push_status(audio.status(), &format!("Import error: {}", e));
                                }
                            }

                            // Pop back to the pane that opened the file browser
                            result.push_nav(NavIntent::Pop);
                        }
                        Err(e) => {
                            eprintln!("Failed to parse .scd file: {}", e);
                            result.push_nav(NavIntent::Pop);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read .scd file: {}", e);
                    result.push_nav(NavIntent::Pop);
                }
            }
        }
        SessionAction::AdjustHumanizeVelocity(delta) => {
            state.session.humanize_velocity = (state.session.humanize_velocity + delta).clamp(0.0, 1.0);
            result.audio_dirty.session = true;
        }
        SessionAction::AdjustHumanizeTiming(delta) => {
            state.session.humanize_timing = (state.session.humanize_timing + delta).clamp(0.0, 1.0);
            result.audio_dirty.session = true;
        }
        SessionAction::ImportVstPlugin(ref path, kind) => {
            use crate::state::vst_plugin::VstPlugin;

            let kind = *kind;

            // Extract display name from filename
            let name = path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "VST Plugin".to_string());

            let plugin = VstPlugin {
                id: 0, // Will be set by registry.add()
                name: name.clone(),
                plugin_path: path.clone(),
                kind,
                params: vec![],
            };

            let _id = state.session.vst_plugins.add(plugin);

            result.push_status(audio.status(), &format!("Imported VST: {}", name));

            result.push_nav(NavIntent::Pop);
            result.audio_dirty.session = true;
        }
    }

    result
}
