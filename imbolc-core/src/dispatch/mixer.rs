use crate::audio::AudioHandle;
use crate::state::automation::AutomationTarget;
use crate::state::{AppState, MixerSelection};
use crate::action::{DispatchResult, MixerAction};

use super::automation::record_automation_point;

pub(super) fn dispatch_mixer(
    action: &MixerAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    let mut result = DispatchResult::none();
    match action {
        MixerAction::Move(delta) => {
            state.mixer_move(*delta);
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                state.instruments.selected = Some(idx);
            }
        }
        MixerAction::Jump(direction) => {
            state.mixer_jump(*direction);
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                state.instruments.selected = Some(idx);
            }
        }
        MixerAction::SelectAt(selection) => {
            state.session.mixer_selection = *selection;
            if let MixerSelection::Instrument(idx) = *selection {
                state.instruments.selected = Some(idx);
            }
        }
        MixerAction::AdjustLevel(delta) => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.level = (instrument.level + delta).clamp(0.0, 1.0);
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.mixer_params = true;
                        if state.automation_recording && state.session.piano_roll.playing {
                            record_target = Some((
                                AutomationTarget::InstrumentLevel(instrument.id),
                                instrument.level,
                            ));
                        }
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.level = (bus.level + delta).clamp(0.0, 1.0);
                        result.audio_dirty.session = true;
                        result.audio_dirty.mixer_params = true;
                    }
                    if let Some(bus) = state.session.bus(id) {
                        let mute = state.session.effective_bus_mute(bus);
                        bus_update = Some((id, bus.level, mute, bus.pan));
                        if state.automation_recording && state.session.piano_roll.playing {
                            record_target = Some((
                                AutomationTarget::BusLevel(id),
                                bus.level,
                            ));
                        }
                    }
                }
                MixerSelection::Master => {
                    state.session.master_level = (state.session.master_level + delta).clamp(0.0, 1.0);
                    result.audio_dirty.session = true;
                    result.audio_dirty.mixer_params = true;
                }
            }
            if audio.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
            // Record automation point
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
        }
        MixerAction::ToggleMute => {
            let mut bus_update: Option<(u8, f32, bool, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.mute = !instrument.mute;
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.mixer_params = true;
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.mute = !bus.mute;
                        result.audio_dirty.session = true;
                        result.audio_dirty.mixer_params = true;
                    }
                    if let Some(bus) = state.session.bus(id) {
                        let mute = state.session.effective_bus_mute(bus);
                        bus_update = Some((id, bus.level, mute, bus.pan));
                    }
                }
                MixerSelection::Master => {
                    state.session.master_mute = !state.session.master_mute;
                    result.audio_dirty.session = true;
                    result.audio_dirty.mixer_params = true;
                }
            }
            if audio.is_running() {
                if let Some((bus_id, level, mute, pan)) = bus_update {
                    let _ = audio.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
        }
        MixerAction::ToggleSolo => {
            let mut bus_updates: Vec<(u8, f32, bool, f32)> = Vec::new();
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.solo = !instrument.solo;
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.mixer_params = true;
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.solo = !bus.solo;
                        result.audio_dirty.session = true;
                        result.audio_dirty.mixer_params = true;
                    }
                }
                MixerSelection::Master => {}
            }
            for bus in &state.session.buses {
                let mute = state.session.effective_bus_mute(bus);
                bus_updates.push((bus.id, bus.level, mute, bus.pan));
            }
            if audio.is_running() {
                for (bus_id, level, mute, pan) in bus_updates {
                    let _ = audio.set_bus_mixer_params(bus_id, level, mute, pan);
                }
            }
        }
        MixerAction::CycleSection => {
            state.session.mixer_cycle_section();
            // When cycling back to Instrument section, sync to global selection
            if let MixerSelection::Instrument(_) = state.session.mixer_selection {
                if let Some(idx) = state.instruments.selected {
                    state.session.mixer_selection = MixerSelection::Instrument(idx);
                }
            }
        }
        MixerAction::CycleOutput => {
            state.mixer_cycle_output();
        }
        MixerAction::CycleOutputReverse => {
            state.mixer_cycle_output_reverse();
        }
        MixerAction::AdjustSend(bus_id, delta) => {
            let bus_id = *bus_id;
            let delta = *delta;
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                    if let Some((send_idx, send)) = instrument.sends.iter_mut().enumerate().find(|(_, s)| s.bus_id == bus_id) {
                        send.level = (send.level + delta).clamp(0.0, 1.0);
                        result.audio_dirty.instruments = true;
                        if state.automation_recording && state.session.piano_roll.playing {
                            record_target = Some((
                                AutomationTarget::SendLevel(instrument.id, send_idx),
                                send.level,
                            ));
                        }
                    }
                }
            }
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
        }
        MixerAction::AdjustPan(delta) => {
            let mut record_target: Option<(AutomationTarget, f32)> = None;
            match state.session.mixer_selection {
                MixerSelection::Instrument(idx) => {
                    if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                        instrument.pan = (instrument.pan + delta).clamp(-1.0, 1.0);
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.mixer_params = true;
                        if state.automation_recording && state.session.piano_roll.playing {
                            let target = AutomationTarget::InstrumentPan(instrument.id);
                            record_target = Some((target.clone(), target.normalize_value(instrument.pan)));
                        }
                    }
                }
                MixerSelection::Bus(id) => {
                    if let Some(bus) = state.session.bus_mut(id) {
                        bus.pan = (bus.pan + delta).clamp(-1.0, 1.0);
                        result.audio_dirty.session = true;
                        result.audio_dirty.mixer_params = true;
                    }
                    if let Some(bus) = state.session.bus(id) {
                        let mute = state.session.effective_bus_mute(bus);
                        if audio.is_running() {
                            let _ = audio.set_bus_mixer_params(id, bus.level, mute, bus.pan);
                        }
                    }
                }
                MixerSelection::Master => {}
            }
            if let Some((target, value)) = record_target {
                record_automation_point(state, target, value);
                result.audio_dirty.automation = true;
            }
        }
        MixerAction::ToggleSend(bus_id) => {
            let bus_id = *bus_id;
            if let MixerSelection::Instrument(idx) = state.session.mixer_selection {
                if let Some(instrument) = state.instruments.instruments.get_mut(idx) {
                    if let Some(send) = instrument.sends.iter_mut().find(|s| s.bus_id == bus_id) {
                        send.enabled = !send.enabled;
                        if send.enabled && send.level <= 0.0 {
                            send.level = 0.5;
                        }
                        result.audio_dirty.instruments = true;
                        result.audio_dirty.routing_instrument = Some(instrument.id);
                    }
                }
            }
        }
    }

    result
}
