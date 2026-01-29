# Architecture

Detailed architecture reference for the tuidaw codebase. See [CLAUDE.md](../CLAUDE.md) for quick reference.

## State Ownership

All state lives in `AppState`, owned by `main.rs` and passed to panes by reference:

```rust
// src/state/mod.rs
pub struct AppState {
    pub strip: StripState,
    pub audio_in_waveform: Option<Vec<f32>>,
}
```

`StripState` is the real workhorse — it contains everything:

```rust
// src/state/strip_state.rs
pub struct StripState {
    pub strips: Vec<Strip>,
    pub selected: Option<usize>,
    pub next_id: StripId,
    pub buses: Vec<MixerBus>,
    pub master_level: f32,
    pub master_mute: bool,
    pub piano_roll: PianoRollState,
    pub mixer_selection: MixerSelection,
    pub automation: AutomationState,
    pub midi_recording: MidiRecordingState,
    pub custom_synthdefs: CustomSynthDefRegistry,
}
```

There is no separate `MixerState` — mixer controls are split between `StripState` (buses, master) and individual `Strip` structs (per-strip level, pan, mute, solo, output target, sends).

## The Strip Model

A `Strip` is the fundamental unit — it combines what were previously separate rack modules (oscillator, filter, effects, output) into a single entity:

```rust
// src/state/strip.rs
pub struct Strip {
    pub id: StripId,
    pub name: String,
    pub source: OscType,           // Saw, Sin, Sqr, Tri, AudioIn, Sampler, Custom
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
    pub effects: Vec<EffectSlot>,
    pub lfo: LfoConfig,
    pub amp_envelope: EnvConfig,
    pub polyphonic: bool,
    pub has_track: bool,           // whether this strip has a piano roll track
    // Integrated mixer controls
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub output_target: OutputTarget,  // Master or Bus(1-8)
    pub sends: Vec<MixerSend>,
    pub sampler_config: Option<SamplerConfig>,
}
```

When a strip is added via `StripState::add_strip(OscType)`:
- A piano roll track is auto-created (unless `AudioIn` source)
- Sampler strips get a default `SamplerConfig`
- Custom synthdef strips get params from the registry

When a strip is removed, its piano roll track is also removed.

## Pane Trait & Rendering

All panes implement the `Pane` trait (`src/ui/pane.rs`):

```rust
pub trait Pane {
    fn id(&self) -> &'static str;
    fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action;
    fn render(&self, g: &mut dyn Graphics, state: &AppState);
    fn keymap(&self) -> &Keymap;
    fn on_enter(&mut self, _state: &AppState) {}
    fn on_exit(&mut self, _state: &AppState) {}
    fn wants_exclusive_input(&self) -> bool { false }
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

Every pane receives `&AppState` for both input handling and rendering. There is no `render_with_state()` workaround — the refactor moved state out of panes and into `AppState`, eliminating the borrow checker issue.

### Registered Panes

| ID | Pane | Key | Purpose |
|----|------|-----|---------|
| `strip` | `StripPane` | `1` | Main view — list of strips with params |
| `piano_roll` | `PianoRollPane` | `2` | Note grid editor with playback |
| `sequencer` | `SequencerPane` | `3` | Song structure (minimal currently) |
| `mixer` | `MixerPane` | `4` | Mixer channels, buses, master |
| `server` | `ServerPane` | `5` | SuperCollider server status/control |
| `home` | `HomePane` | — | Welcome screen |
| `add` | `AddPane` | — | Strip creation menu |
| `strip_edit` | `StripEditPane` | — | Edit strip params/effects/filter |
| `frame_edit` | `FrameEditPane` | — | Session settings (BPM, key, etc.) |
| `file_browser` | `FileBrowserPane` | — | File selection for imports |
| `help` | `HelpPane` | `?` | Context-sensitive keybinding help |

### Pane Communication

Panes communicate exclusively through `Action` values. A pane's `handle_input()` returns an `Action`, which is dispatched by `dispatch::dispatch_action()` in `src/dispatch.rs`. This function receives `&mut AppState`, `&mut PaneManager`, `&mut AudioEngine`, etc. and can mutate anything.

For cross-pane data passing (e.g., opening the strip editor with a specific strip's data), the dispatch function uses `PaneManager::get_pane_mut::<T>()` to downcast and configure the target pane before switching to it.

## Borrow Patterns

When dispatch needs data from one pane to configure another:

```rust
// Extract data first, then use — the two borrows don't overlap
let strip_data = state.strip.strip(*id).cloned();
if let Some(strip) = strip_data {
    if let Some(edit) = panes.get_pane_mut::<StripEditPane>("strip_edit") {
        edit.set_strip(&strip);
    }
    panes.switch_to("strip_edit", state);
}
```

The key constraint: extracted data must be owned (cloned), not a reference. Each `get_pane_mut` borrows `&mut self` on `PaneManager`, so you can't hold two simultaneously.

## Action Dispatch Flow

```
User Input
  → main.rs: poll_event()
  → main.rs: check global keys (Ctrl-Q, Ctrl-S, number keys)
  → PaneManager::handle_input() → active pane's handle_input()
  → returns Action
  → dispatch::dispatch_action() matches on Action
  → mutates AppState / calls AudioEngine / configures panes
```

The `dispatch_action()` function (`src/dispatch.rs`) handles all ~50 action variants. It returns `bool` — `true` means quit.

## Audio Engine

Located in `src/audio/`. Communicates with SuperCollider (scsynth) via OSC over UDP.

### Key Components

- `AudioEngine` (`engine.rs`) — manages synth nodes, bus allocation, routing
- `OscClient` (`osc_client.rs`) — OSC message/bundle sending
- `BusAllocator` (`bus_allocator.rs`) — audio/control bus allocation

### SuperCollider Groups

```
GROUP_SOURCES    = 100  — all source synths (oscillators, samplers)
GROUP_PROCESSING = 200  — filters, effects, mixer processing
GROUP_OUTPUT     = 300  — output synths
```

### Strip → Synth Mapping

Each strip maps to multiple SuperCollider synth nodes (source, filter, effects, output, LFO, envelope). `AudioEngine::rebuild_strip_routing()` tears down and rebuilds the full signal chain when strips change.

### OSC Communication

- `OscClient::send_message()` — fire-and-forget single message
- `OscClient::set_params_bundled()` — multiple params in one timestamped bundle
- `OscClient::send_bundle()` — multiple messages in one timestamped bundle
- `osc_time_from_now(offset_secs)` — NTP timetag for sample-accurate scheduling

Use bundles for timing-sensitive operations (note events). Individual messages are fine for UI parameter changes.

## Playback Engine

Lives in `src/playback.rs`, called from the main event loop every frame (~16ms):

1. Compute elapsed real time since last frame
2. Convert to ticks: `seconds * (bpm / 60) * ticks_per_beat`
3. Advance playhead, handle loop wrapping
4. Scan all tracks for notes starting in the elapsed tick range
5. Send note-on bundles with sub-frame timestamps via OSC
6. Decrement active note durations, send note-off when expired

Tick resolution: 480 ticks per beat. Notes are sent as OSC bundles with NTP timetags for sample-accurate scheduling.

## Persistence

SQLite database via `rusqlite`. Implementation in `src/state/persistence.rs`.

### What's Persisted

Comprehensive — the full state survives save/load:
- Strips (source type, name, filter, LFO, envelope, polyphonic, mixer controls)
- Source parameters (with type: float/int/bool)
- Effects chain (type, params, enabled, ordering)
- Sends (per-strip bus sends with level and enabled)
- Filter modulation sources (LFO, envelope, or strip-param cross-modulation)
- Mixer buses (name, level, pan, mute, solo)
- Master level and mute
- Piano roll tracks and notes
- Musical settings (BPM, time signature, key, scale, tuning, loop points)
- Automation lanes and points (with curve types)
- Sampler configs (buffer, loop mode, slices)
- Custom synthdef registry (name, params, source path)

### What's NOT Persisted

- UI selection state (strip selection, mixer selection)
- Playback position
- MIDI recording state
- Audio engine state (rebuilt on connect)
