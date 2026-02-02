use crate::audio::AudioHandle;
use crate::state::automation::AutomationTarget;
use crate::state::{AppState, ClipboardContents};
use crate::action::{AutomationAction, DispatchResult};

/// Minimum value change threshold for recording (0.5%)
const RECORD_VALUE_THRESHOLD: f32 = 0.005;
/// Minimum tick delta between recorded points (1/10th beat)
const RECORD_MIN_TICK_DELTA: u32 = 48;

pub(super) fn dispatch_automation(
    action: &AutomationAction,
    state: &mut AppState,
    audio: &mut AudioHandle,
) -> DispatchResult {
    let mut result = DispatchResult::none();
    match action {
        AutomationAction::AddLane(target) => {
            state.session.automation.add_lane(target.clone());
            result.audio_dirty.automation = true;
        }
        AutomationAction::RemoveLane(id) => {
            state.session.automation.remove_lane(*id);
            result.audio_dirty.automation = true;
        }
        AutomationAction::ToggleLaneEnabled(id) => {
            if let Some(lane) = state.session.automation.lane_mut(*id) {
                lane.enabled = !lane.enabled;
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::AddPoint(lane_id, tick, value) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                lane.add_point(*tick, *value);
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::RemovePoint(lane_id, tick) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                lane.remove_point(*tick);
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::MovePoint(lane_id, old_tick, new_tick, new_value) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                lane.remove_point(*old_tick);
                lane.add_point(*new_tick, *new_value);
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::SetCurveType(lane_id, tick, curve) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                if let Some(point) = lane.point_at_mut(*tick) {
                    point.curve = *curve;
                    result.audio_dirty.automation = true;
                }
            }
        }
        AutomationAction::SelectLane(delta) => {
            if *delta > 0 {
                state.session.automation.select_next();
            } else {
                state.session.automation.select_prev();
            }
        }
        AutomationAction::ClearLane(id) => {
            if let Some(lane) = state.session.automation.lane_mut(*id) {
                lane.points.clear();
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::ToggleRecording => {
            state.automation_recording = !state.automation_recording;
        }
        AutomationAction::RecordValue(target, value) => {
            // Find or create lane for this target
            let lane_id = state.session.automation.add_lane(target.clone());
            let playhead = state.session.piano_roll.playhead;
            if let Some(lane) = state.session.automation.lane_mut(lane_id) {
                lane.add_point(playhead, *value);
                result.audio_dirty.automation = true;
            }
            // Apply immediately for audio feedback
            if audio.is_running() {
                // Map normalized value to actual range
                let (min, max) = target.default_range();
                let actual_value = min + value * (max - min);
                let _ = audio.apply_automation(target, actual_value, &state.instruments, &state.session);
            }
        }
        AutomationAction::DeletePointsInRange(lane_id, start_tick, end_tick) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                lane.points.retain(|p| p.tick < *start_tick || p.tick >= *end_tick);
                result.audio_dirty.automation = true;
            }
        }
        AutomationAction::CopyPoints(lane_id, start_tick, end_tick) => {
            if *start_tick < *end_tick {
                if let Some(lane) = state.session.automation.lane(*lane_id) {
                    let mut points = Vec::new();
                    for point in &lane.points {
                        if point.tick >= *start_tick && point.tick <= *end_tick {
                            points.push((point.tick - start_tick, point.value));
                        }
                    }
                    if !points.is_empty() {
                        state.clipboard.contents = Some(ClipboardContents::AutomationPoints { points });
                    }
                }
            }
        }
        AutomationAction::PastePoints(lane_id, anchor_tick, points) => {
            if let Some(lane) = state.session.automation.lane_mut(*lane_id) {
                for (tick_offset, value) in points {
                    let tick = *anchor_tick + tick_offset;
                    lane.add_point(tick, *value);
                }
                result.audio_dirty.automation = true;
            }
        }
    }

    result
}

/// Record an automation point with thinning
pub(super) fn record_automation_point(state: &mut AppState, target: AutomationTarget, value: f32) {
    let playhead = state.session.piano_roll.playhead;
    let lane_id = state.session.automation.add_lane(target);

    if let Some(lane) = state.session.automation.lane_mut(lane_id) {
        // Point thinning: skip if value changed less than threshold and tick delta is small
        if let Some(last) = lane.points.last() {
            let value_delta = (value - last.value).abs();
            let tick_delta = if playhead > last.tick {
                playhead - last.tick
            } else {
                last.tick - playhead
            };
            if value_delta < RECORD_VALUE_THRESHOLD && tick_delta < RECORD_MIN_TICK_DELTA {
                return;
            }
        }
        lane.add_point(playhead, value);
    }
}
