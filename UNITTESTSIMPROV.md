# Unit Test Coverage Plan (to 50%)

## Phase 0: Baseline & Reporting (0–1%)
- Generate per-file coverage report (HTML) to identify hotspots.
- Keep policy consistent: include `src/`, exclude `target/`.

## Phase 1: Pure Logic Tests (target +10–15%)
Focus on modules with minimal side effects:
- `src/state/*`: `piano_roll`, `automation`, `instrument_state`, `drum_sequencer`, `sampler`, `music`.
  - Edge cases: empty tracks, tick bounds, loop wrapping, automation interpolation curves, track order.
- `src/scd_parser.rs`: regex edge cases, defaults, internal param filtering.
- `src/midi/mod.rs`: invalid/short messages, unknown status bytes.
- `src/ui/keybindings.rs`: modifier parsing, invalid key strings (expected behavior).
- `src/config.rs`: invalid/missing values and fallback behavior.

## Phase 2: Pane Action Logic (target +8–12%)
Cover `handle_action` and state transitions (avoid rendering):
- `src/panes/*`: `instrument_pane`, `mixer_pane`, `piano_roll_pane`, `sequencer_pane`, `file_browser_pane`, `frame_edit_pane`.
- Use dummy `InputEvent` and assert returned `Action` + state mutations.

## Phase 3: Persistence & Integration (target +4–8%)
- Expand `src/state/persistence.rs` tests:
  - Multiple instruments, effects, sends, automation lanes, sampler configs.
  - Round-trip: save -> load -> compare key fields.
  - Optional: save -> load -> save invariance.

## Phase 4: Audio Routing Test Seams (target +8–12%)
This is the likely requirement to reach ~50%.
- Extract a pure routing plan builder from `AudioEngine::rebuild_instrument_routing`.
- Add a lightweight `OscClient` trait + mock to validate emitted messages.
- Test bus allocation, LFO routing, effect ordering, and output targeting.

## Expected Trajectory
- After Phase 1 + 2: ~35–40%
- After Phase 3: ~40–45%
- After Phase 4: ~48–55%
