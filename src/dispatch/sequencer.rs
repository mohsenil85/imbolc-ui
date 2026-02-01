use crate::audio::AudioEngine;
use crate::panes::FileBrowserPane;
use crate::state::drum_sequencer::DrumPattern;
use crate::state::sampler::Slice;
use crate::state::AppState;
use crate::ui::{ChopperAction, PaneManager, SequencerAction};

use super::helpers::compute_waveform_peaks;

pub(super) fn dispatch_sequencer(
    action: &SequencerAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        SequencerAction::ToggleStep(pad_idx, step_idx) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(step) = seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .and_then(|s| s.get_mut(*step_idx))
                {
                    step.active = !step.active;
                }
            }
        }
        SequencerAction::AdjustVelocity(pad_idx, step_idx, delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(step) = seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .and_then(|s| s.get_mut(*step_idx))
                {
                    step.velocity = (step.velocity as i16 + *delta as i16).clamp(1, 127) as u8;
                }
            }
        }
        SequencerAction::ClearPad(pad_idx) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                for step in seq
                    .pattern_mut()
                    .steps
                    .get_mut(*pad_idx)
                    .iter_mut()
                    .flat_map(|s| s.iter_mut())
                {
                    step.active = false;
                }
            }
        }
        SequencerAction::ClearPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let len = seq.pattern().length;
                *seq.pattern_mut() = DrumPattern::new(len);
            }
        }
        SequencerAction::CyclePatternLength => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let lengths = [8, 16, 32, 64];
                let current = seq.pattern().length;
                let idx = lengths.iter().position(|&l| l == current).unwrap_or(0);
                let new_len = lengths[(idx + 1) % lengths.len()];
                let old_pattern = seq.pattern().clone();
                let mut new_pattern = DrumPattern::new(new_len);
                for (pad_idx, old_steps) in old_pattern.steps.iter().enumerate() {
                    for (step_idx, step) in old_steps.iter().enumerate() {
                        if step_idx < new_len {
                            new_pattern.steps[pad_idx][step_idx] = step.clone();
                        }
                    }
                }
                *seq.pattern_mut() = new_pattern;
            }
        }
        SequencerAction::NextPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.current_pattern = (seq.current_pattern + 1) % seq.patterns.len();
            }
        }
        SequencerAction::PrevPattern => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.current_pattern = if seq.current_pattern == 0 {
                    seq.patterns.len() - 1
                } else {
                    seq.current_pattern - 1
                };
            }
        }
        SequencerAction::AdjustPadLevel(pad_idx, delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                    pad.level = (pad.level + delta).clamp(0.0, 1.0);
                }
            }
        }
        SequencerAction::PlayStop => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                seq.playing = !seq.playing;
                if !seq.playing {
                    seq.current_step = 0;
                    seq.step_accumulator = 0.0;
                }
            }
        }
        SequencerAction::LoadSample(pad_idx) => {
            if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                fb.open_for(
                    crate::ui::FileSelectAction::LoadDrumSample(*pad_idx),
                    None,
                );
            }
            panes.push_to("file_browser", &*state);
        }
        SequencerAction::LoadSampleResult(pad_idx, path) => {
            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let buffer_id = seq.next_buffer_id;
                seq.next_buffer_id += 1;

                if audio_engine.is_running() {
                    let _ = audio_engine.load_sample(buffer_id, &path_str);
                }

                if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                    pad.buffer_id = Some(buffer_id);
                    pad.path = Some(path_str);
                    pad.name = name;
                }
            }

            panes.pop(&*state);
        }
    }
}

pub(super) fn dispatch_chopper(
    action: &ChopperAction,
    state: &mut AppState,
    panes: &mut PaneManager,
    audio_engine: &mut AudioEngine,
) {
    match action {
        ChopperAction::LoadSample => {
            if let Some(fb) = panes.get_pane_mut::<FileBrowserPane>("file_browser") {
                fb.open_for(crate::ui::FileSelectAction::LoadChopperSample, None);
            }
            panes.push_to("file_browser", &*state);
        }
        ChopperAction::LoadSampleResult(path) => {
            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            // Compute waveform peaks from WAV file
            let (peaks, duration_secs) = compute_waveform_peaks(&path_str);

            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let buffer_id = seq.next_buffer_id;
                seq.next_buffer_id += 1;

                if audio_engine.is_running() {
                    let _ = audio_engine.load_sample(buffer_id, &path_str);
                }

                let initial_slice = Slice::full(0);
                seq.chopper = Some(crate::state::drum_sequencer::ChopperState {
                    buffer_id: Some(buffer_id),
                    path: Some(path_str),
                    name,
                    slices: vec![initial_slice],
                    selected_slice: 0,
                    next_slice_id: 1,
                    waveform_peaks: peaks,
                    duration_secs,
                });
            }

            // Only pop if we're at the standalone file browser (pushed via LoadSample action).
            // When using the embedded browser inside the chopper pane, we're already where we want to be.
            if panes.active().id() == "file_browser" {
                panes.pop(&*state);
            }
        }
        ChopperAction::AddSlice(cursor_pos) => {
            let cursor_pos = *cursor_pos;
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    // Find which slice contains cursor_pos
                    if let Some(idx) = chopper.slices.iter().position(|s| s.start <= cursor_pos && s.end > cursor_pos) {
                        let old_end = chopper.slices[idx].end;
                        chopper.slices[idx].end = cursor_pos;

                        let new_id = chopper.next_slice_id;
                        chopper.next_slice_id += 1;
                        let new_slice = Slice::new(new_id, cursor_pos, old_end);
                        chopper.slices.insert(idx + 1, new_slice);
                        chopper.selected_slice = idx + 1;
                    }
                }
            }
        }
        ChopperAction::RemoveSlice => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if chopper.slices.len() > 1 {
                        let idx = chopper.selected_slice;
                        let removed = chopper.slices.remove(idx);
                        if idx > 0 {
                            // Extend previous slice's end to cover gap
                            chopper.slices[idx - 1].end = removed.end;
                            chopper.selected_slice = idx - 1;
                        } else if !chopper.slices.is_empty() {
                            // Extend next slice's start to cover gap
                            chopper.slices[0].start = removed.start;
                            chopper.selected_slice = 0;
                        }
                    }
                }
            }
        }
        ChopperAction::AssignToPad(pad_idx) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                let assign_data = seq.chopper.as_ref().and_then(|c| {
                    c.slices.get(c.selected_slice).map(|s| (c.buffer_id, s.start, s.end))
                });
                if let Some((buffer_id, start, end)) = assign_data {
                    if let Some(pad) = seq.pads.get_mut(*pad_idx) {
                        pad.buffer_id = buffer_id;
                        pad.slice_start = start;
                        pad.slice_end = end;
                        // Copy name from chopper
                        if let Some(chopper) = &seq.chopper {
                            pad.name = format!("{} {}", chopper.name, chopper.selected_slice + 1);
                            pad.path = chopper.path.clone();
                        }
                    }
                }
            }
        }
        ChopperAction::AutoSlice(n) => {
            let n = *n;
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    chopper.slices.clear();
                    for i in 0..n {
                        let start = i as f32 / n as f32;
                        let end = (i + 1) as f32 / n as f32;
                        let id = chopper.next_slice_id;
                        chopper.next_slice_id += 1;
                        chopper.slices.push(Slice::new(id, start, end));
                    }
                    chopper.selected_slice = 0;
                }
            }
        }
        ChopperAction::PreviewSlice => {
            if let Some(instrument) = state.instruments.selected_instrument() {
                if let Some(seq) = &instrument.drum_sequencer {
                    if let Some(chopper) = &seq.chopper {
                        if let Some(slice) = chopper.slices.get(chopper.selected_slice) {
                            if let Some(buffer_id) = chopper.buffer_id {
                                if audio_engine.is_running() {
                                    let _ = audio_engine.play_drum_hit_to_instrument(
                                        buffer_id, 0.8, instrument.id,
                                        slice.start, slice.end,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        ChopperAction::SelectSlice(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if !chopper.slices.is_empty() {
                        let len = chopper.slices.len() as i8;
                        let new_idx = (chopper.selected_slice as i8 + delta).rem_euclid(len) as usize;
                        chopper.selected_slice = new_idx;
                    }
                }
            }
        }
        ChopperAction::NudgeSliceStart(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if let Some(slice) = chopper.slices.get_mut(chopper.selected_slice) {
                        slice.start = (slice.start + delta).clamp(0.0, slice.end - 0.001);
                    }
                }
            }
        }
        ChopperAction::NudgeSliceEnd(delta) => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &mut seq.chopper {
                    if let Some(slice) = chopper.slices.get_mut(chopper.selected_slice) {
                        slice.end = (slice.end + delta).clamp(slice.start + 0.001, 1.0);
                    }
                }
            }
        }
        ChopperAction::CommitAll => {
            if let Some(seq) = state.instruments.selected_drum_sequencer_mut() {
                if let Some(chopper) = &seq.chopper {
                    let assignments: Vec<_> = chopper.slices.iter().enumerate()
                        .take(crate::state::drum_sequencer::NUM_PADS)
                        .map(|(i, s)| (i, chopper.buffer_id, s.start, s.end, chopper.name.clone(), chopper.path.clone()))
                        .collect();
                    for (i, buffer_id, start, end, name, path) in assignments {
                        if let Some(pad) = seq.pads.get_mut(i) {
                            pad.buffer_id = buffer_id;
                            pad.slice_start = start;
                            pad.slice_end = end;
                            pad.name = format!("{} {}", name, i + 1);
                            pad.path = path;
                        }
                    }
                }
            }
            panes.pop(&*state);
        }
        ChopperAction::MoveCursor(_) => {
            // Cursor tracked locally in pane
        }
    }
}
