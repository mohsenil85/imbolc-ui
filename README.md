# ilex

![ilex](ilex.png)

ilex is a terminal-based digital audio workstation (DAW) built in Rust. The UI is a TUI (ratatui) and the audio engine runs on SuperCollider (scsynth) via OSC. It is optimized for keyboard-first instrument editing, sequencing, and mixing inside the terminal.

## Highlights

- Instrument strips with a source, filter, FX chain, and output routing.
- Built-in sources: classic waves plus noise, pulse, supersaw, FM, phase mod, pluck, formant, gendy, chaos, additive, wavetable; Audio In, Bus In, Pitched Sampler, Kit (12-pad drum machine), custom SynthDefs, and VST instruments.
- Filters: low-pass, high-pass, band-pass. Effects: delay, reverb, gate, tape comp, sidechain comp.
- Sequencing: piano roll with per-note velocity, loop points, 480 ticks/beat, plus a step sequencer for kits and a sample chopper for slicing/assigning pads.
- Mixer with level/pan/mute/solo, 8 buses, sends, and master control.
- Performance modes: computer keyboard piano/pad, two-digit instrument select, fully keybound UI.
- Audio backend: dedicated audio thread with ~1ms tick resolution, decoupled from UI. scsynth DSP with OSC bundles and NTP timetags for sample-accurate note timing.
- Recording: toggle master recording to WAV and view audio input or recorded waveform.
- Persistence: project model stored in SQLite; the audio graph is rebuilt on load.

## Requirements

- Rust toolchain (edition 2021; tested with 1.70+).
- SuperCollider: `scsynth` on PATH. For custom SynthDefs, `sclang` should also be available.
- macOS device selection uses `system_profiler`. Other platforms may need extra work for device enumeration.

## Build and run

```bash
cargo run --release
```

ilex will attempt to auto-start scsynth. Use the Server pane (F5) to manage devices, compile/load synthdefs, or restart the server.

## Low-latency timing

- Dedicated audio thread ticks at ~1ms and never waits on UI render/input.
- The sequencer converts tick offsets to seconds and sends OSC bundles with NTP timetags, so scsynth schedules notes sample-accurately ahead of time.
- Jitter in the UI thread does not affect playback timing; the audio thread advances based on elapsed time and schedules notes accordingly.
- Input polling runs at 2 ms and rendering at ~60 fps, but audio scheduling is not gated on either.

## UI tour

- F1 - Instruments: list and manage instruments, press Enter to edit.
- F2 - Piano Roll / Sequencer / Waveform (context-driven):
  - Kit instruments open the step sequencer.
  - Audio In / Bus In instruments open the waveform view.
  - Other instruments open the piano roll.
- F3 - Track: timeline overview (early/WIP).
- F4 - Mixer: instrument and bus levels, sends, mute/solo.
- F5 - Server: scsynth status, device selection, synthdef build/load, master recording.
- F6 - Logo.
- F7 - Automation: lanes and point editing.
- Ctrl+f - Frame Edit: BPM, time signature, tuning, key/scale, snap.
- ? - Context help for the active pane.
- Ctrl+s / Ctrl+l - Save/load the default project.
- / - Toggle performance mode (piano or pad keyboard depending on instrument).

## Keybindings and config

- Default bindings live in `keybindings.toml` and are embedded at build time.
- Override bindings in `~/.config/ilex/keybindings.toml`.
- Default musical settings can be overridden in `~/.config/ilex/config.toml`.

## Project files

- Default project file: `~/.config/ilex/default.sqlite`
- Custom synthdefs: `~/.config/ilex/synthdefs/`
- Audio device preferences: `~/.config/ilex/audio_devices.json`
- Recordings: `master_<timestamp>.wav` in the current working directory

## Docs

- `docs/architecture.md` — state ownership and pane/dispatch flow
- `docs/sc-engine-architecture.md` — SuperCollider engine details
- `docs/vst3-support-roadmap.md` — current VST3 plan and UI targets
- `docs/vst-integration.md` — legacy notes (superseded)

## Architecture (from TECHDEETS)

ilex uses an MVU-inspired architecture adapted for a TUI with a dedicated audio engine.

### Core components

- AppState: single source of truth (SessionState, InstrumentState, and UI-related state).
- Panes: stateless views that render from AppState and emit Action enums.
- dispatch module (`ilex-core/src/dispatch`): central event handler; mutates AppState and drives AudioHandle (command interface to the audio thread).
- AudioEngine: manages scsynth and mirrors InstrumentState into a concrete DSP graph. Runs on a dedicated audio thread, communicated via MPSC commands.

### Data flow

User input -> Pane::handle_action / Pane::handle_raw_input -> Action -> dispatch -> mutate state / AudioHandle -> [MPSC channel] -> AudioThread -> send OSC -> render

## Audio engine details

- Scheduling runs on a dedicated audio thread at ~1ms tick resolution, fully decoupled from the UI. The main loop polls input every 2 ms and renders at ~60 fps. Note events are sent as OSC bundles with future NTP timetags for sample-accurate playback. UI jank cannot affect playback timing.
- BusAllocator deterministically assigns audio/control buses and resets on project load or engine restart.
- SynthDefs live in `synthdefs/` and are loaded into scsynth at startup. Execution order is enforced via groups:
  - 100: Sources
  - 200: Processing
  - 300: Output
  - 400: Record

### Signal flow and routing

Each instrument is realized as a chain of SuperCollider nodes:

Source -> Filter -> FX Chain -> Output (Master or Bus)

- Polyphonic sources spawn per-voice groups and sum into a shared source bus.
- Audio In and Bus In are monophonic sources with persistent synths.
- Mixer sends and bus targets are handled in the output stage.

### Polyphony and voice chaining

- Each note spawns a voice group with an `ilex_midi` control synth feeding a source synth.
- Voices for an instrument feed a single shared filter/FX chain (paraphonic-ish).
- Voice stealing is FIFO at 16 voices per instrument.

### Modulation

- SynthDefs expose *_mod_in inputs; LFOs write to control buses and are selected via `Select.kr` inside SynthDefs.
- The audio engine currently wires LFOs to filter cutoff; additional targets are outlined in `ilex-core/src/state/instrument.rs`.

## Persistence

Projects are stored as SQLite databases. The database captures the project model (instruments, effects, mixer buses, notes, automation, drum pads, and settings). The audio graph is reconstructed from this model on load.

## Known limitations

- UI and input are on the main thread; input polling (2 ms) is decoupled from rendering (~60 fps). Audio scheduling runs on a separate thread at ~1ms resolution, so UI load does not affect playback timing.
- Voice stealing is FIFO; smarter strategies are not implemented.
- Parameter smoothing is limited; rapid changes can cause zippering.
- LFO target wiring beyond filter cutoff is still in progress.

## Testing

```bash
cargo test
# tmux-based E2E tests are ignored by default
cargo test -- --ignored
```

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.
