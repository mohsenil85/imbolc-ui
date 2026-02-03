use super::{ServerPane, ServerPaneFocus};
use crate::state::AppState;
use crate::ui::{Action, InputEvent, KeyCode, ServerAction};

impl ServerPane {
    pub(super) fn handle_action_impl(&mut self, action: &str, _event: &InputEvent, _state: &AppState) -> Action {
        match action {
            "start" => Action::Server(ServerAction::Start {
                input_device: self.selected_input_device(),
                output_device: self.selected_output_device(),
            }),
            "stop" => Action::Server(ServerAction::Stop),
            "connect" => Action::Server(ServerAction::Connect),
            "disconnect" => Action::Server(ServerAction::Disconnect),
            "compile" => Action::Server(ServerAction::CompileSynthDefs),
            "compile_vst" => Action::Server(ServerAction::CompileVstSynthDefs),
            "load_synthdefs" => Action::Server(ServerAction::LoadSynthDefs),
            "record_master" => Action::Server(ServerAction::RecordMaster),
            "refresh_devices" => {
                self.refresh_devices();
                self.refresh_log();
                if self.server_running {
                    Action::Server(ServerAction::Restart {
                        input_device: self.selected_input_device(),
                        output_device: self.selected_output_device(),
                    })
                } else {
                    Action::None
                }
            }
            "next_section" => {
                self.cycle_focus();
                Action::None
            }
            _ => Action::None,
        }
    }

    pub(super) fn handle_raw_input_impl(&mut self, event: &InputEvent, _state: &AppState) -> Action {
        match self.focus {
            ServerPaneFocus::OutputDevice => {
                let count = self.output_devices().len() + 1;
                match event.key {
                    KeyCode::Up => {
                        self.selected_output = if self.selected_output == 0 {
                            count - 1
                        } else {
                            self.selected_output - 1
                        };
                        return Action::None;
                    }
                    KeyCode::Down => {
                        self.selected_output = (self.selected_output + 1) % count;
                        return Action::None;
                    }
                    KeyCode::Enter => {
                        self.save_config();
                        if self.server_running {
                            self.device_config_dirty = false;
                            return Action::Server(ServerAction::Restart {
                                input_device: self.selected_input_device(),
                                output_device: self.selected_output_device(),
                            });
                        } else {
                            self.device_config_dirty = true;
                            return Action::None;
                        }
                    }
                    _ => {}
                }
            }
            ServerPaneFocus::InputDevice => {
                let count = self.input_devices().len() + 1;
                match event.key {
                    KeyCode::Up => {
                        self.selected_input = if self.selected_input == 0 {
                            count - 1
                        } else {
                            self.selected_input - 1
                        };
                        return Action::None;
                    }
                    KeyCode::Down => {
                        self.selected_input = (self.selected_input + 1) % count;
                        return Action::None;
                    }
                    KeyCode::Enter => {
                        self.save_config();
                        if self.server_running {
                            self.device_config_dirty = false;
                            return Action::Server(ServerAction::Restart {
                                input_device: self.selected_input_device(),
                                output_device: self.selected_output_device(),
                            });
                        } else {
                            self.device_config_dirty = true;
                            return Action::None;
                        }
                    }
                    _ => {}
                }
            }
            ServerPaneFocus::Controls => {}
        }

        Action::None
    }
}
