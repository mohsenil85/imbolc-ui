use crate::state::AppState;
use crate::action::{DispatchResult, NavIntent};

pub(super) fn handle_add(
    state: &mut AppState,
    source_type: crate::state::SourceType,
) -> DispatchResult {
    state.add_instrument(source_type);
    let mut result = DispatchResult::with_nav(NavIntent::SwitchTo("instrument_edit"));
    result.audio_dirty.instruments = true;
    result.audio_dirty.piano_roll = true;
    result.audio_dirty.routing = true;
    result
}

pub(super) fn handle_delete(
    state: &mut AppState,
    inst_id: crate::state::InstrumentId,
) -> DispatchResult {
    state.remove_instrument(inst_id);
    let mut result = if state.instruments.instruments.is_empty() {
        DispatchResult::with_nav(NavIntent::SwitchTo("add"))
    } else {
        DispatchResult::none()
    };
    result.audio_dirty.instruments = true;
    result.audio_dirty.piano_roll = true;
    result.audio_dirty.automation = true;
    result.audio_dirty.routing = true;
    result
}

pub(super) fn handle_edit(
    state: &mut AppState,
    id: crate::state::InstrumentId,
) -> DispatchResult {
    state.instruments.editing_instrument_id = Some(id);
    DispatchResult::with_nav(NavIntent::SwitchTo("instrument_edit"))
}

pub(super) fn handle_update(
    state: &mut AppState,
    update: &crate::action::InstrumentUpdate,
) -> DispatchResult {
    if let Some(instrument) = state.instruments.instrument_mut(update.id) {
        instrument.source = update.source.clone();
        instrument.source_params = update.source_params.clone();
        instrument.filter = update.filter.clone();
        instrument.eq = update.eq.clone();
        instrument.effects = update.effects.clone();
        instrument.amp_envelope = update.amp_envelope.clone();
        instrument.polyphonic = update.polyphonic;
        instrument.active = update.active;
    }
    let mut result = DispatchResult::none();
    result.audio_dirty.instruments = true;
    result.audio_dirty.routing_instrument = Some(update.id);
    result
}
