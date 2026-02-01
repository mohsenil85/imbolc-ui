# VST3 Support Roadmap

Goal: move ilex from "can load a VST" to "full support" with a usable TUI
workflow (parameter browsing, automation, state recall, and advanced features).

Note: docs/vst-integration.md is legacy notes from an earlier prototype. This
file reflects the current Rust codebase and the intended direction.

## Current support (today)

Implemented in the Rust codebase and SC engine:

- VSTPlugin wrapper SynthDefs exist: `synthdefs/compile_vst.scd` generates
  `ilex_vst_instrument` and `ilex_vst_effect`.
- VST registry data model exists (`src/state/vst_plugin.rs`) and is persisted in
  SQLite (`vst_plugins`, `vst_plugin_params`).
- Add pane can import a `.vst` / `.vst3` bundle as a VST instrument. The file
  browser treats bundle directories as selectable.
- Audio engine opens a VST plugin in SuperCollider by sending a `/u_cmd` to the
  VSTPlugin UGen when routing is rebuilt.
- VST instruments receive note-on/off via `/u_cmd` MIDI messages from the
  sequencer.

What is missing or stubbed:

- No plugin scanning or cataloging; only manual import by file path.
- No parameter discovery, UI, or editing.
- No automation lanes for VST parameters.
- No plugin state save/restore (chunk state), only registry metadata.
- No preset/program handling.
- No param groups, MIDI learn, or latency reporting/compensation.
- VST effects are not wired into the UI flow yet (only wrapper + type exist).

## Parameter browser (idea)

Because we cannot open native plugin GUIs in a TUI, a generic parameter browser
is the core user-facing surface for VST3 support.

Minimum viable shape:

- A searchable list of parameters (name, current value, default value).
- A detail panel showing range, unit, and a live control widget (slider/knob).
- Actions: set value, reset to default, favorite, and "add automation lane".
- Optional compact/favorites view to keep large plugins usable.

Example layout (sketch):

```
+-- VST Params: Serum ---------------------------------------------+
| / cutoff  reso  env  lfo  filter  osc  fx                       |
|                                                                  |
| > 001 Cutoff            0.72 [Hz]                               |
|   002 Resonance         0.30 [%]                                |
|   003 Env Amount        0.55                                    |
|                                                                  |
| Range: 20..20000 Hz  Default: 2000  Automation: off             |
| [Left/Right] adjust  [a] add lane  [f] favorite  [r] reset       |
+------------------------------------------------------------------+
```

## Target: "full support"

"Full support" means a VST3 can be loaded, controlled, automated, saved,
reloaded, and used reliably inside a session with no external GUI.

That implies:

- Complete parameter enumeration and editing.
- Automation recording and playback for VST parameters.
- Plugin state persistence and restore.
- Preset/program management.
- Param grouping and MIDI learn for faster control.
- Latency reporting (and compensation where possible).

## Plan A: Param list + automation + state

This phase makes VSTs truly usable in projects.

1) Parameter discovery
   - Query VSTPlugin for parameter metadata when a plugin is opened.
   - Store param specs (name, default, unit, normalized range) in
     `VstPlugin` and/or a per-instance state.

2) Parameter browser UI
   - Read-only list at first, then editable values.
   - Add search and favorites.
   - Wire edits to `/u_cmd` param set messages.

3) Parameter automation
   - New automation target: `VstParam { instance_id, param_index }`.
   - Playback: send param changes during tick (sample-accurate if possible,
     otherwise block-accurate).

4) Plugin state save/restore
   - Store a per-instance "state blob" (chunk) provided by the plugin.
   - Persist with the session so reloads are deterministic.

## Plan B: Presets + param groups + MIDI learn + latency

This phase makes VSTs feel first-class and fast to use.

1) Presets / programs
   - Load and save preset files.
   - List plugin programs and allow quick switching.

2) Param groups
   - Surface VST3 units/groups in the UI.
   - Allow browsing by group (osc/filter/mod/etc.).

3) MIDI learn
   - Map external MIDI CCs to VST parameters.
   - Store mappings per instance or per plugin.

4) Latency
   - Query plugin latency and expose it in UI.
   - If possible, compensate in playback (or at least report).

## Notes

- The SC VSTPlugin path is the current integration strategy; the UI/automation
  design should remain agnostic so a future native host can reuse it.
- Keep all VST metadata and per-instance state in the session DB so projects are
  portable and reload correctly.
