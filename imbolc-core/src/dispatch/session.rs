use std::path::PathBuf;
use std::sync::mpsc::Sender;
use crate::audio::AudioHandle;
use crate::scd_parser;
use crate::state::{AppState, CustomSynthDef, ParamSpec};
use crate::action::{DispatchResult, IoFeedback, NavIntent, SessionAction};

use super::server::{compile_synthdef, config_synthdefs_dir};
use super::default_rack_path;

fn dispatch_save(
    path: PathBuf,
    state: &mut AppState,
    audio: &AudioHandle,
    io_tx: &Sender<IoFeedback>,
    result: &mut DispatchResult,
) {
    // Sync piano roll time_signature from session before cloning
    state.session.piano_roll.time_signature = state.session.time_signature;

    let session = state.session.clone();
    let instruments = state.instruments.clone();
    let tx = io_tx.clone();
    let save_id = state.io_generation.next_save();

    std::thread::spawn(move || {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let res = crate::state::persistence::save_project(&path, &session, &instruments)
            .map(|_| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("default")
                    .to_string()
            })
            .map_err(|e| e.to_string());

        let _ = tx.send(IoFeedback::SaveComplete { id: save_id, path, result: res });
    });

    result.push_status(audio.status(), "Saving...");
}

fn dispatch_load(
    path: PathBuf,
    state: &mut AppState,
    audio: &AudioHandle,
    io_tx: &Sender<IoFeedback>,
    result: &mut DispatchResult,
) {
    let tx = io_tx.clone();
    let load_id = state.io_generation.next_load();

    std::thread::spawn(move || {
        let res = if path.exists() {
            crate::state::persistence::load_project(&path)
                .map(|(session, instruments)| {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("default")
                        .to_string();
                    (session, instruments, name)
                })
                .map_err(|e| e.to_string())
        } else {
            Err("Project file not found".to_string())
        };

        let _ = tx.send(IoFeedback::LoadComplete { id: load_id, path, result: res });
    });

    result.push_status(audio.status(), "Loading...");
}

pub(super) fn dispatch_session(
    action: &SessionAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
    io_tx: &Sender<IoFeedback>,
) -> DispatchResult {
    let mut result = DispatchResult::none();

    match action {
        SessionAction::Save => {
            let path = state.project_path.clone().unwrap_or_else(default_rack_path);
            dispatch_save(path, state, audio, io_tx, &mut result);
        }
        SessionAction::SaveAs(ref path) => {
            let path = path.clone();
            dispatch_save(path, state, audio, io_tx, &mut result);
            result.push_nav(NavIntent::ConditionalPop("save_as"));
        }
        SessionAction::Load => {
            let path = state.project_path.clone().unwrap_or_else(default_rack_path);
            dispatch_load(path, state, audio, io_tx, &mut result);
            result.push_nav(NavIntent::ConditionalPop("confirm"));
            result.push_nav(NavIntent::ConditionalPop("project_browser"));
        }
        SessionAction::LoadFrom(ref path) => {
            let path = path.clone();
            dispatch_load(path, state, audio, io_tx, &mut result);
            result.push_nav(NavIntent::ConditionalPop("confirm"));
            result.push_nav(NavIntent::ConditionalPop("project_browser"));
        }
        SessionAction::NewProject => {
            // Signal main loop to handle the full reset (undo history, audio sync, etc.)
            // We set a flag via DispatchResult; main loop does the actual reset.
            result.new_project = true;
            result.push_nav(NavIntent::ConditionalPop("confirm"));
            result.push_nav(NavIntent::ConditionalPop("project_browser"));
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
            let path = path.clone();
            let tx = io_tx.clone();
            let import_id = state.io_generation.next_import_synthdef();
            
            std::thread::spawn(move || {
                // Read and parse the .scd file
                let res = match std::fs::read_to_string(&path) {
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

                                // Copy the .scd file to the config synthdefs directory
                                let config_dir = config_synthdefs_dir();
                                let _ = std::fs::create_dir_all(&config_dir);

                                // Copy .scd file
                                if let Some(filename) = path.file_name() {
                                    let dest = config_dir.join(filename);
                                    let _ = std::fs::copy(&path, &dest);
                                }

                                // Compile the synthdef
                                match compile_synthdef(&path, &config_dir, &synthdef_name) {
                                    Ok(scsyndef_path) => Ok((custom, synthdef_name, scsyndef_path)),
                                    Err(e) => Err(format!("Failed to compile synthdef: {}", e)),
                                }
                            }
                            Err(e) => Err(format!("Failed to parse .scd file: {}", e)),
                        }
                    }
                    Err(e) => Err(format!("Failed to read .scd file: {}", e)),
                };
                
                let _ = tx.send(IoFeedback::ImportSynthDefComplete { id: import_id, result: res });
            });

            result.push_status(audio.status(), "Importing SynthDef...");
            result.push_nav(NavIntent::Pop);
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
        SessionAction::ToggleMasterMute => {
            state.session.master_mute = !state.session.master_mute;
            result.audio_dirty.session = true;
            result.audio_dirty.mixer_params = true;
        }
    }

    result
}
