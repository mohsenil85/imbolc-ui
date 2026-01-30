# ilex

![ilex](ilex.png)

A terminal-based Digital Audio Workstation built in Rust. Wire up oscillators, filters, and effects in a modular rack, sequence notes in a piano roll, and mix it all down — without leaving the terminal.

Uses [ratatui](https://github.com/ratatui/ratatui) for the TUI and [SuperCollider](https://supercollider.github.io/) (scsynth) for real-time audio synthesis via OSC.

## Features

- **Modular rack** — Add oscillators, filters, LFOs, effects, and output modules. Connect them with a visual signal routing system.
- **Piano roll** — Place and edit notes with per-note velocity. BPM-based timing at 480 ticks/beat resolution with loop support.
- **Mixer** — Channel strips with level, pan, mute, and solo. Output modules auto-assign to mixer channels.
- **Real-time synthesis** — All audio runs through SuperCollider. OSC bundles with NTP timetags for sample-accurate scheduling.
- **Custom SynthDefs** — Import your own `.scd` instruments. Parameters are auto-discovered and editable in the rack.
- **Persistence** — Sessions saved as SQLite databases (`.ilex` files). Modules, parameters, connections all preserved.
- **Keyboard-driven** — Every action is a keypress. No mouse needed. Vim-style navigation throughout.

## Prerequisites

- **Rust** 1.70+
- **SuperCollider** — [Install scsynth](https://supercollider.github.io/downloads). The server (`scsynth`) must be available on your PATH.

## Build & Run

```bash
cargo build --release
cargo run --release
```

## Module Types

| Category    | Modules                          | Description                              |
|-------------|----------------------------------|------------------------------------------|
| Sources     | Saw, Sine, Square, Triangle      | Classic waveform oscillators             |
| Sources     | MIDI                             | Pitched instrument, driven by piano roll |
| Sources     | Sampler                          | Trigger audio samples                    |
| Filters     | Low-Pass Filter                  | Subtractive filtering with cutoff mod    |
| Modulation  | LFO                              | Low-frequency oscillator for modulation  |
| Effects     | Delay                            | Time-based echo effect                   |
| Output      | Output                           | Routes audio to mixer channel + hardware |

## Signal Routing

Modules communicate through audio and control buses. Connect an output port to an input port to route signal between modules. Multiple writers to the same bus are summed automatically.

```
  ┌──────────┐        ┌──────────┐        ┌──────────┐
  │  Saw Osc │──out──>│──in  LPF │──out──>│──in  Out │
  └──────────┘        └──────────┘        └──────────┘
                           ^
  ┌──────────┐             │
  │   LFO    │──out──>cutoff_mod
  └──────────┘
```

Execution order is determined by topological sort — sources run first, then processing, then outputs.

## Keybindings

### Global

| Key       | Action              |
|-----------|---------------------|
| `Ctrl-q`  | Quit                |
| `Ctrl-s`  | Save session        |
| `Ctrl-l`  | Load session        |
| `1`-`9`   | Switch pane         |
| `.`       | Master mute toggle  |
| `?`       | Help                |

### Rack

| Key            | Action                    |
|----------------|---------------------------|
| `j` / `k`      | Navigate modules          |
| `a`            | Add module                |
| `d`            | Delete module             |
| `e`            | Edit parameters           |
| `c`            | Connect mode              |
| `x`            | Disconnect                |

### Connect Mode

| Key            | Action                    |
|----------------|---------------------------|
| `j` / `k`      | Navigate modules          |
| `Tab` / `h` / `l` | Cycle ports           |
| `Enter`        | Confirm selection         |
| `Esc`          | Cancel                    |

### Edit Mode

| Key            | Action                    |
|----------------|---------------------------|
| `j` / `k`      | Navigate parameters       |
| `h` / `l`      | Decrease / increase value |
| `Esc`          | Return to rack            |

Press `?` in any pane to see its full keybinding reference.

## Architecture

```
src/
├── main.rs            Event loop, action dispatch, playback engine
├── dispatch.rs        Action dispatch logic
├── panes/             UI views
│   ├── strip_pane     Main rack display
│   ├── piano_roll     Note editor
│   ├── mixer          Channel strips
│   ├── add_pane       Module picker
│   ├── sequencer      Timeline
│   └── ...
├── state/             Application state
│   ├── strip          Modules, connections, mixer channels
│   ├── piano_roll     Tracks, notes, transport
│   ├── persistence    SQLite save/load
│   └── custom_synthdef  User instrument registry
├── audio/             SuperCollider integration
│   ├── engine         Synth management, bus allocation, routing
│   └── osc_client     OSC message/bundle transport
└── ui/                TUI framework
    ├── pane           Pane trait, Action enum
    ├── keymap         Keybinding system
    ├── style          Colors and theming
    └── frame          Frame buffer, rendering
```

### Data Flow

```
User Input -> Pane::handle_input() -> Action -> main.rs dispatch -> mutate state / send OSC
```

Panes never mutate state directly. All mutations flow through the Action enum and are handled centrally in the dispatch loop.

### Audio Engine

SuperCollider runs as an external process (`scsynth`). The DAW communicates entirely over OSC/UDP:

- Modules map to synth nodes, organized into source/processing/output groups
- Connections allocate buses — audio buses for signal, control buses for modulation
- `rebuild_routing()` reconstructs the full audio graph on topology changes
- Bundles with NTP timetags enable sub-frame scheduling precision (~60fps render loop, but note timing is not quantized to frames)

## Persistence

Sessions are stored as SQLite databases at `~/.config/ilex/rack.ilex`.

Currently persisted:
- Modules with all parameters
- Connections between modules
- Session metadata

## Testing

```bash
cargo test --bin ilex    # unit tests
cargo test                 # all tests
```

## Dependencies

| Crate      | Purpose                       |
|------------|-------------------------------|
| ratatui    | Terminal UI rendering         |
| crossterm  | Terminal I/O backend          |
| rosc       | OSC protocol for SuperCollider|
| rusqlite   | SQLite persistence            |
| midir      | MIDI input                    |
| serde      | Serialization                 |
| regex      | SCD file parsing              |
| dirs       | Platform config directories   |

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.
