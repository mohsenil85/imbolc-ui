# CLAUDE.md

Guide for AI agents working on this codebase.

## What This Is

A terminal-based DAW (Digital Audio Workstation) in Rust. Uses ratatui for TUI rendering and SuperCollider via OSC for audio synthesis. Instruments combine an oscillator source, filter, effects chain, LFO, envelope, and mixer controls into a single unit. Instruments are sequenced via piano roll.

## Directory Structure

```
src/
  main.rs          — Binary event loop, global keybindings, render loop
  panes/           — UI views (see docs/architecture.md for full list)
  ui/              — TUI framework (pane trait, keymap, input, style, widgets)
  setup.rs         — Auto-startup for SuperCollider

ilex-core/src/
  action.rs        — Action enums + DispatchResult
  audio/           — SuperCollider OSC client and audio engine
    handle.rs        — AudioHandle (main-thread interface) and AudioThread (audio thread)
    commands.rs      — AudioCmd and AudioFeedback enums
  config.rs        — TOML config loading (musical defaults)
  dispatch/        — Action handler (all state mutation happens here)
  scd_parser.rs    — SuperCollider .scd file parser
  state/           — All application state
    mod.rs           — AppState (top-level), re-exports
    instrument.rs    — Instrument, InstrumentId, SourceType, FilterType, EffectType, LFO, envelope types
    instrument_state.rs — InstrumentState (instruments, selection, persistence helpers)
    session.rs       — SessionState (mixer, global settings, automation)
    persistence/     — SQLite save/load implementation
    piano_roll.rs    — PianoRollState, Track, Note
    automation.rs    — AutomationState, lanes, points, curve types
    sampler.rs       — SamplerConfig, SampleRegistry, slices
    custom_synthdef.rs — CustomSynthDef registry and param specs
    music.rs         — Key, Scale, musical theory types
    midi_recording.rs — MIDI recording state, CC mappings
    param.rs         — Param, ParamValue (Float/Int/Bool)
  midi/            — MIDI utilities
```

## Key Types

| Type | Location | What It Is |
|------|----------|------------|
| `AppState` | `ilex-core/src/state/mod.rs` | Top-level state, owned by `main.rs`, passed to panes as `&AppState` |
| `InstrumentState` | `ilex-core/src/state/instrument_state.rs` | Collection of instruments and selection state |
| `SessionState` | `ilex-core/src/state/session.rs` | Global session data: buses, mixer, piano roll, automation |
| `Instrument` | `ilex-core/src/state/instrument.rs` | One instrument: source + filter + effects + LFO + envelope + mixer |
| `InstrumentId` | `ilex-core/src/state/instrument.rs` | `u32` — unique identifier for instruments |
| `SourceType` | `ilex-core/src/state/instrument.rs` | Oscillator/Source types (Saw/Sin/etc, AudioIn, BusIn, PitchedSampler, Kit, Custom, VST) |
| `Action` | `ilex-core/src/action.rs` | Action enum (re-exported in `src/ui/pane.rs`) |
| `Pane` | `src/ui/pane.rs` | Trait: `id()`, `handle_action()`, `handle_raw_input()`, `handle_mouse()`, `render()`, `keymap()` |
| `PaneManager` | `src/ui/pane.rs` | Owns all panes, manages active pane, coordinates input |
| `AudioHandle` | `ilex-core/src/audio/handle.rs` | Main-thread interface; sends AudioCmd via MPSC channel to audio thread |

## Critical Patterns

See [docs/architecture.md](docs/architecture.md) for detailed architecture, state ownership, borrow patterns, and persistence.

### Action Dispatch

Panes return `Action` values from `handle_action()` / `handle_raw_input()`. `ilex-core/src/dispatch/` matches on them and mutates state. Panes never mutate state directly.

When adding a new action:
1. Add variant to `Action` enum in `ilex-core/src/action.rs`
2. Return it from the pane's `handle_action()` (or `handle_raw_input()` if it bypasses layers)
3. Handle it in `dispatch::dispatch_action()` in `ilex-core/src/dispatch/mod.rs`

### Navigation

Pane switching uses function keys: `F1`=instrument, `F2`=piano roll / sequencer / waveform (context-driven), `F3`=track, `F4`=mixer, `F5`=server, `F6`=logo, `F7`=automation. `` ` ``/`~` for back/forward. `?` for context-sensitive help. `Ctrl+f` opens the frame settings.

Number keys select instruments: `1`-`9` select instruments 1-9, `0` selects 10, `_` enters two-digit instrument selection.

### Pane Registration

New panes must be:
1. Created in `src/panes/` and added to `src/panes/mod.rs`
2. Registered in `main.rs`: `panes.add_pane(Box::new(MyPane::new()));`
3. Given a number-key binding in the global key match block (if navigable)

## UI Framework API

### Keymap

```rust
Keymap::new()
    .bind('q', "action_name", "Description")
    .bind_key(KeyCode::Up, "action_name", "Description")
    .bind_ctrl('s', "action_name", "Description")
    .bind_alt('x', "action_name", "Description")
    .bind_ctrl_key(KeyCode::Left, "action_name", "Desc")
    .bind_shift_key(KeyCode::Right, "action_name", "Desc")
```

Shift bindings only exist for special keys (e.g. `Shift+Right`). For shifted
characters, bind the literal char (`?`, `A`, `+`) rather than a Shift+ variant.

### Colors

`Color::new(r, g, b)` for custom RGB. Named constants: `Color::WHITE`, `Color::PINK`, `Color::SELECTION_BG`, `Color::MIDI_COLOR`, `Color::METER_LOW`. **No `Color::rgb()`** — use `Color::new()`.

### Pane Sizing

Use `ui::layout_helpers::center_rect(area, width, height)` to center a sub-rect. Most panes derive an inner rect from the frame and then place content relative to that.

## Build & Test

```bash
cargo build               # compile
cargo test --bin ilex   # unit tests
cargo test                # all tests including e2e
```

## Configuration

TOML-based configuration system with embedded defaults and optional user overrides.

- **Musical defaults:** `config.toml` (embedded) + `~/.config/ilex/config.toml` (user override)
- **Keybindings:** `keybindings.toml` (embedded) + `~/.config/ilex/keybindings.toml` (user override)
- Config loading: `ilex-core/src/config.rs` — `Config::load()` parses embedded defaults, layers user overrides
- Keybinding loading: `src/ui/keybindings.rs` — same embedded + user override pattern
- User override files are optional; missing fields fall back to embedded defaults

Musical defaults (`[defaults]` section): `bpm`, `key`, `scale`, `tuning_a4`, `time_signature`, `snap`

## Persistence

- Format: SQLite database (`.ilex` / `.sqlite`)
- Save/load: `save_project()` / `load_project()` in `ilex-core/src/state/persistence/mod.rs`
- Default path: `~/.config/ilex/default.sqlite`
- Persists: instruments, params, effects, filters, sends, modulations, buses, mixer, piano roll, automation, sampler configs, custom synthdefs, drum sequencer, midi settings

## LSP Integration (CCLSP)

Configured as MCP server (`cclsp.json` + `.mcp.json`). Provides rust-analyzer access. Prefer LSP tools over grep for navigating Rust code — they understand types, scopes, and cross-file references.

## Detailed Documentation

- [docs/architecture.md](docs/architecture.md) — state ownership, instrument model, pane rendering, action dispatch, borrow patterns
- [docs/audio-routing.md](docs/audio-routing.md) — bus model, insert vs send, node ordering
- [docs/keybindings.md](docs/keybindings.md) — keybinding philosophy and conventions
- [docs/ai-coding-affordances.md](docs/ai-coding-affordances.md) — patterns that help AI agents work faster
- [docs/sc-engine-architecture.md](docs/sc-engine-architecture.md) — SuperCollider engine modules
- [docs/polyphonic-voice-allocation.md](docs/polyphonic-voice-allocation.md) — voice allocation design
- [docs/custom-synthdef-plan.md](docs/custom-synthdef-plan.md) — custom SynthDef import system
- [docs/sqlite-persistence.md](docs/sqlite-persistence.md) — persistence schema design
- [docs/ai-integration.md](docs/ai-integration.md) — planned Haiku integration

## Plans

Save implementation plans in `./plans/` with descriptive filenames (e.g., `plans/midi-clock-sync.md`, `plans/sample-browser-redesign.md`). Use names that clearly describe the feature or change being planned.

## Comment Box

Log difficulties, friction points, or things that gave you trouble in `COMMENTBOX.md` at the project root. This helps identify recurring pain points and areas where the codebase or documentation could be improved.
