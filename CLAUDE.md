# CLAUDE.md

Guide for AI agents working on this codebase.

## What This Is

A terminal-based DAW (Digital Audio Workstation) in Rust. Uses ratatui for TUI rendering and SuperCollider via OSC for audio synthesis. Modules are wired together in a virtual rack, routed through a mixer, and sequenced via piano roll.

## Architecture

```
src/
  main.rs          — Event loop, action dispatch, playback engine, pane rendering
  state/           — All application state (rack, modules, mixer, piano roll)
  panes/           — UI views (rack, mixer, piano roll, sequencer, server, etc.)
  ui/              — TUI framework (pane trait, keymap, input, style, widgets)
  audio/           — SuperCollider OSC client and audio engine
  core/            — Shared types (actions, effects)
```

### Data Flow

```
User Input → Pane::handle_input() → Action enum → main.rs match → mutate state / call audio engine
```

All state lives in `RackState` (owned by `RackPane`). Other panes that need state (mixer, piano roll) get it via `render_with_state()` — see "Render With State Pattern" below.

### Key Types

- `ModuleId` = `u32` — unique identifier for rack modules
- `ModuleType` — enum of all module types (Midi, SawOsc, Lpf, Output, etc.)
- `Action` — enum in `src/ui/pane.rs` for all dispatchable actions
- `RackState` — central state container in `src/state/rack.rs`
- `PianoRollState` — tracks, notes, transport in `src/state/piano_roll.rs`
- `MixerState` — channels, buses, sends in `src/state/mixer.rs`

## Important Patterns

### Render With State Pattern

Some panes need data from `RackState` but don't own it (only `RackPane` does). These panes implement `render_with_state()` as a public method and get special-cased in main.rs:

```rust
// main.rs render section
if active_id == "mixer" {
    let rack_state = panes.get_pane_mut::<RackPane>("rack").map(|r| r.rack().clone());
    if let Some(rack) = rack_state {
        panes.get_pane_mut::<MixerPane>("mixer").unwrap().render_with_state(&mut frame, &rack);
    }
} else if active_id == "piano_roll" {
    // similar — clones piano_roll state, passes to pane
}
```

The `Pane::render()` trait method is a fallback that renders with empty/default state. **Any new pane that needs rack state must be added to this block in main.rs.**

This is a workaround for Rust's borrow checker — you can't hold two `&mut` references from `PaneManager` simultaneously. The pattern is: extract data (clone), drop the borrow, then pass to the target pane.

### Action Dispatch

Pane input handlers return `Action` values. `main.rs` matches on them and mutates state. Panes never mutate `RackState` directly — they return actions.

When adding a new action:
1. Add variant to `Action` enum in `src/ui/pane.rs`
2. Return it from the pane's `handle_input()`
3. Handle it in the `match &action` block in `main.rs`

### Auto-Assignment

When modules are added to the rack:
- `Output` modules → auto-assigned to a free mixer channel
- `Midi` modules → auto-assigned a piano roll track

Both happen in `RackState::add_module()` / `remove_module()`.

## UI Framework API

### Keymap

```rust
Keymap::new()
    .bind('q', "action_name", "Description")           // char key
    .bind_key(KeyCode::Up, "action_name", "Description") // special key
    .bind_ctrl('s', "action_name", "Description")       // Ctrl+char
    .bind_alt('x', "action_name", "Description")        // Alt+char
    .bind_ctrl_key(KeyCode::Left, "action_name", "Desc") // Ctrl+special key
```

**There is no `bind_shift_key`.** Shift state is available on `event.modifiers.shift` — handle it manually before keymap lookup if needed.

### Colors

```rust
Color::new(r, g, b)  // custom RGB
Color::WHITE          // named constants
Color::PINK
Color::SELECTION_BG   // UI semantic colors
Color::MIDI_COLOR     // module type colors
Color::METER_LOW      // meter colors
```

No `Color::rgb()` — use `Color::new()`.

### Pane Sizing

All main panes use `Rect::centered(width, height, 97, 29)` for consistent sizing within the outer frame. Follow this convention for new panes.

### Pane Registration

New panes must be:
1. Created in `src/panes/` and added to `src/panes/mod.rs`
2. Registered in `main.rs`: `panes.add_pane(Box::new(MyPane::new()));`
3. Given an F-key binding in the global F-key match block (if navigable)

## Audio Engine

### OSC Communication

All audio is handled by SuperCollider (scsynth) via OSC over UDP.

- `OscClient::send_message()` — fire-and-forget single message
- `OscClient::set_params_bundled()` — multiple params in one timestamped bundle (for note-on)
- `OscClient::send_bundle()` — multiple messages in one timestamped bundle
- `osc_time_from_now(offset_secs)` — NTP timetag for sample-accurate scheduling

Use bundles for anything timing-sensitive (note events). Individual `set_param` is fine for UI knob tweaks.

### Module → Synth Mapping

Each rack module gets a SuperCollider synth node. `AudioEngine::node_map` maps `ModuleId → node_id`. Modules communicate via buses (audio and control), allocated during `rebuild_routing()`.

## Persistence

- File format: `.tuidaw` (SQLite database)
- Schema: see `docs/sqlite-persistence.md`
- Save/load in `RackState::save()` / `RackState::load()` in `src/state/rack.rs`
- Currently persists: modules, params, connections, next_id
- NOT yet persisted (`#[serde(skip)]`): mixer state, piano roll state, UI selection
- Save path: `~/.config/tuidaw/rack.tuidaw`

## Build & Test

```bash
cargo build        # compile
cargo test --bin tuidaw  # unit tests (55 tests)
cargo test         # all tests including e2e (e2e may fail if tmux not configured)
```

## LSP Integration (CCLSP)

This project has CCLSP configured as an MCP server, providing rust-analyzer LSP access to Claude Code. Config lives in `cclsp.json` (LSP server settings) and `.mcp.json` (MCP registration).

Available tools:
- `find_definition` — jump to where a symbol is defined
- `find_references` — find all usages of a symbol across the workspace
- `find_workspace_symbols` — search for symbols by name
- `get_diagnostics` — get compiler errors/warnings for a file
- `get_hover` — type info and docs for a symbol at a position
- `rename_symbol` / `rename_symbol_strict` — rename a symbol across the codebase
- `find_implementation` — find implementations of a trait/interface
- `get_incoming_calls` / `get_outgoing_calls` — call hierarchy

Prefer these tools over grep for navigating Rust code — they understand types, scopes, and cross-file references.

Note: rust-analyzer needs time to index on first launch. Some tools may time out until indexing completes.

## Existing Docs

- `docs/audio-routing.md` — bus model, insert vs send, node ordering, mixer architecture
- `docs/sqlite-persistence.md` — SQLite schema design, sharing, presets
- `docs/keybindings.md` — keybinding philosophy and conventions
- `docs/ai-integration.md` — planned Haiku integration for natural language commands
