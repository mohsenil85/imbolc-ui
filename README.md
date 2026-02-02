# imbolc

![imbolc](imbolc.png)

imbolc is a terminal-based digital audio workstation (DAW) written in Rust. The UI is a TUI (ratatui) and the audio engine runs on SuperCollider (scsynth) via OSC. It is optimized for keyboard-first instrument editing, sequencing, and mixing inside the terminal.

## Quick start

- Install Rust (edition 2021) and SuperCollider (scsynth on PATH; sclang needed for synthdef compilation).
- Run: `cargo run --release`
- Use `F5` for server controls, `F1`-`F7` to switch panes, and `?` for context help.

Developer mode (UI only):

```bash
IMBOLC_NO_AUDIO=1 cargo run
```

## Features

- Instrument model: source + filter + FX chain + LFO + envelope + mixer routing.
- Sources: classic waves + noise, sync, FM/phase mod, pluck, formant, gendy, chaos, additive, wavetable; audio in/bus in; pitched sampler; kit; custom SynthDefs; VST instruments (experimental).
- Filters: low-pass, high-pass, band-pass.
- Effects: delay, reverb, gate, tape comp, sidechain comp. (More effects are defined in `synthdefs/compile.scd`; run the server "Compile SynthDefs" action to regenerate `.scsyndef` files.)
- Sequencing: piano roll with per-note velocity, loop points, 480 ticks/beat; kit step sequencer; sample chopper.
- Mixer: channel/bus levels, pan, mute/solo, 8 buses, sends, master control.
- Automation lanes (including VST params when discovered).
- Recording: master/input to WAV with waveform view.
- Low-latency playback: dedicated audio thread (~1ms tick) with OSC bundles and NTP timetags for sample-accurate scheduling.

## UI tour (defaults)

- `F1` Instruments: list/manage instruments, `Enter` to edit.
- `F2` Piano Roll / Sequencer / Waveform (context-driven).
- `F3` Track: timeline overview (WIP).
- `F4` Mixer: levels, pan, mute/solo, sends.
- `F5` Server: scsynth status, device selection, synthdef build/load, recording.
- `F6` Logo.
- `F7` Automation: lanes and point editing.
- `Ctrl+f` Frame Edit: BPM, time signature, tuning, key/scale, snap.
- `?` Context help for the active pane.
- `/` Toggle performance mode (piano/pad keyboard depending on instrument).
- `Ctrl+s` / `Ctrl+l` Save/load default project.

The canonical keybinding list lives in `keybindings.toml` and is surfaced in-app via `?`.

## VST support (experimental)

VST support is routed through SuperCollider's VSTPlugin UGen and is still evolving.

What works today:
- Manual import of `.vst` / `.vst3` bundles for instruments and effects (no scanning/catalog yet).
- VST instruments are hosted as persistent nodes; note-on/off is sent via `/u_cmd` MIDI messages.
- VST effects can be inserted in instrument FX chains.
- A VST parameter pane exists (search, adjust, reset, add automation lane).

Current gaps:
- Parameter discovery replies from VSTPlugin are not wired yet (the UI is present, but `discover` does not currently populate params).
- No parameter UI for VST effects (only VST instruments have a param pane today).
- No preset/program browser; VST state save/restore is not surfaced in the UI yet.
- No param groups, MIDI learn, or latency reporting/compensation.

Setup notes:
- Install the VSTPlugin extension in SuperCollider.
- Generate the wrapper synthdefs by running `sclang synthdefs/compile_vst.scd`, then load synthdefs from the Server pane.

## Configuration & files

- Defaults: `config.toml` and `keybindings.toml` (embedded at build time).
- Overrides: `~/.config/imbolc/config.toml`, `~/.config/imbolc/keybindings.toml`.
- Project file: `~/.config/imbolc/default.sqlite`.
- Custom synthdefs: `~/.config/imbolc/synthdefs/`.
- Audio device prefs: `~/.config/imbolc/audio_devices.json`.
- scsynth log: `~/.config/imbolc/scsynth.log`.
- Recordings: `master_<timestamp>.wav` in the current working directory.

macOS device enumeration uses `system_profiler`; other platforms may need extra work for device selection.

## Repo map

- `src/` - TUI app, panes, input layers, render loop.
- `imbolc-core/` - state model, dispatch, audio engine, persistence.
- `synthdefs/` - SuperCollider synth definitions (compiled `.scsyndef`).
- `docs/` - architecture, audio routing, persistence, and roadmaps.

## Docs

- `docs/architecture.md` - state ownership, panes, dispatch flow.
- `docs/audio-routing.md` - buses, sends, and mixer routing.
- `docs/sc-engine-architecture.md` - SC engine modules and design notes.
- `docs/sqlite-persistence.md` - DB schema and persistence model.
- `docs/vst3-support-roadmap.md` - VST plan and current status.
- `docs/ai-coding-affordances.md` - AI-friendly patterns and gotchas.

## Build & test

```bash
cargo ck         # fast typecheck (alias)
cargo build
cargo test --bin imbolc
cargo test
# tmux-based E2E tests are ignored by default
cargo test -- --ignored
```

## License

This project is licensed under the GNU GPL v3.0. See [LICENSE](LICENSE) for details.
