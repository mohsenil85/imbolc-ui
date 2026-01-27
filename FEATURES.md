# Feature Requests

## 1. Keybinding Remap: Number Keys for Pane Navigation
- `1` → Rack, `2` → Piano Roll, `3` → Sequencer, `4` → Mixer, `5` → Server, etc.
- Relocate Help (currently F1) to `?` key (global)
- Free up F-keys for other uses

## 2. F1: Frame Focus Mode
- F1 enters a "frame focus" mode where the user can edit frame-level values
- Frame values: BPM, tuning, time signature, etc.
- Probably a modal overlay or inline editing in the top bar

## 3. Meter Display on Frame
- Add a level meter to the outer frame (off to the right)
- Should show real-time audio output level

## 4. Sequencer: Note Duration Grid Selection
- In the sequencer view, allow switching between note durations for placement
- Quarter notes, eighth notes, sixteenth notes, etc.
- Keybind to cycle or select grid resolution

## 5. Patch View: Tree View (Big One)
- Replace or augment the current rack/patch view with a `tree`-style display
- Exploit the chain-like nature of SC signal flow
- Show module connections as a tree hierarchy (source → processing → output)
- Will likely require iteration to get right
- Example:
  ```
  Output-3
  └── Lpf-2
      └── SawOsc-1
          └── Midi-0
  ```

## 6. Refactor main.rs
- main.rs is getting large — break into smaller files
- Candidates: playback tick logic, action dispatch, pane rendering, app setup

## 7. Linter Cleanup
- Fix existing compiler warnings (unused imports, dead code, etc.)
