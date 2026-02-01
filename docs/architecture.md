# Architecture

Detailed architecture reference for the ilex codebase. See [CLAUDE.md](../CLAUDE.md) for quick reference.
For VST3 plans and UI targets, see `docs/vst3-support-roadmap.md`.

## State Ownership

All state lives in `AppState`, owned by `main.rs` and passed to panes by reference:

```rust
// src/state/mod.rs
pub struct AppState {
    pub session: SessionState,
    pub instruments: InstrumentState,
    pub audio_in_waveform: Option<Vec<f32>>,
    pub recorded_waveform: Option<Vec<f32>>,
    pub pending_recording_path: Option<PathBuf>,
    pub keyboard_layout: KeyboardLayout,
    pub recording: bool,
    pub recording_secs: u64,
}
```

`InstrumentState` (formerly StripState) contains the instruments:

```rust
// src/state/instrument_state.rs
pub struct InstrumentState {
    pub instruments: Vec<Instrument>,
    pub selected: Option<usize>,
    pub next_id: InstrumentId,
    pub next_sampler_buffer_id: u32,
}
```

`SessionState` contains global settings and other state:

```rust
// src/state/session.rs
pub struct SessionState {
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

## The Instrument Model

An `Instrument` (formerly Strip) is the fundamental unit — it combines what were previously separate rack modules (oscillator, filter, effects, output) into a single entity:

```rust
// src/state/instrument.rs
pub struct Instrument {
    pub id: InstrumentId,
    pub name: String,
    pub source: SourceType,        // Saw, Sin, Sqr, Tri, AudioIn, BusIn, PitchedSampler, Kit, Custom
    pub source_params: Vec<Param>,
    pub filter: Option<FilterConfig>,
    pub effects: Vec<EffectSlot>,
    pub lfo: LfoConfig,
    pub amp_envelope: EnvConfig,
    pub polyphonic: bool,
    // Integrated mixer controls
    pub level: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub active: bool,
    pub output_target: OutputTarget,  // Master or Bus(1-8)
    pub sends: Vec<MixerSend>,
    pub sampler_config: Option<SamplerConfig>,
    pub drum_sequencer: Option<DrumSequencerState>,
}
```

When an instrument is added:
- A piano roll track is auto-created
- Sampler instruments get a default `SamplerConfig`
- Kit instruments get a `DrumSequencerState`
- Custom synthdef instruments get params from the registry

## Pane Trait & Rendering

All panes implement the `Pane` trait (`src/ui/pane.rs`):

```rust
pub trait Pane {
    fn id(&self) -> &'static str;
    fn handle_action(&mut self, action: &str, event: &InputEvent, state: &AppState) -> Action;
    fn handle_raw_input(&mut self, event: &InputEvent, state: &AppState) -> Action;
    fn handle_mouse(&mut self, event: &MouseEvent, area: RatatuiRect, state: &AppState) -> Action;
    fn render(&self, area: RatatuiRect, buf: &mut Buffer, state: &AppState);
    fn keymap(&self) -> &Keymap;
    fn on_enter(&mut self, _state: &AppState) {}
    fn on_exit(&mut self, _state: &AppState) {}
    fn toggle_performance_mode(&mut self, _state: &AppState) -> ToggleResult { ToggleResult::NotSupported }
    fn activate_piano(&mut self) {}
    fn activate_pad(&mut self) {}
    fn deactivate_performance(&mut self) {}
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

Every pane receives `&AppState` for input handling and rendering. `handle_action()`
is called only when the layer stack resolves a key to an action string; otherwise
`handle_raw_input()` receives the raw event (e.g., for text input).

### Registered Panes

| ID | Pane | Key | Purpose |
|----|------|-----|---------|
| `instrument` | `InstrumentPane` | `F1` | Main view — list of instruments with params |
| `piano_roll` | `PianoRollPane` | `F2` | Note grid editor with playback |
| `sequencer` | `SequencerPane` | `F2` (context) | Drum sequencer / song structure |
| `mixer` | `MixerPane` | `F4` | Mixer channels, buses, master |
| `server` | `ServerPane` | `F5` | SuperCollider server status/control |
| `track` | `TrackPane` | `F3` | Timeline overview (WIP) |
| `logo` | `LogoPane` | `F6` | Logo splash |
| `waveform` | `WaveformPane` | `F2` (context) | Recorded/input waveform view |
| `home` | `HomePane` | — | Welcome screen |
| `add` | `AddPane` | — | Instrument creation menu |
| `instrument_edit` | `InstrumentEditPane` | — | Edit instrument params/effects/filter |
| `frame_edit` | `FrameEditPane` | `Ctrl+f` | Session settings (BPM, key, etc.) |
| `file_browser` | `FileBrowserPane` | — | File selection for imports |
| `sample_chopper` | `SampleChopperPane` | — | Slice audio and assign pads |
| `help` | `HelpPane` | `?` | Context-sensitive keybinding help |

### Pane Communication

Panes communicate exclusively through `Action` values. A pane's `handle_action()` or
`handle_raw_input()` returns an `Action`, which is dispatched by
`dispatch::dispatch_action()` in `src/dispatch.rs`. This function receives
`&mut AppState`, `&mut PaneManager`, `&mut AudioEngine`, etc. and can mutate anything.

For cross-pane data passing (e.g., opening the editor with a specific instrument's data), the dispatch function uses `PaneManager::get_pane_mut::<T>()` to downcast and configure the target pane before switching to it.

## Borrow Patterns

When dispatch needs data from one pane to configure another:

```rust
// Extract data first, then use — the two borrows don't overlap
let inst_id = state.instruments.selected.map(|idx| state.instruments.instruments[idx].id);
if let Some(id) = inst_id {
    if let Some(edit) = panes.get_pane_mut::<InstrumentEditPane>("instrument_edit") {
        edit.set_instrument(id);
    }
    panes.switch_to("instrument_edit", state);
}
```

The key constraint: extracted data must be owned (cloned), not a reference. Each `get_pane_mut` borrows `&mut self` on `PaneManager`, so you can't hold two simultaneously.

## Action Dispatch Flow

```
User Input
  → main.rs: poll_event()
  → layer_stack.resolve(event) → Action string or Blocked/Unresolved
  → handle_global_action(...) for global layer actions
  → Pane::handle_action(action, event, state) OR Pane::handle_raw_input(event, state)
  → returns Action
  → dispatch::dispatch_action() matches on Action
  → mutates AppState / calls AudioEngine / configures panes
```

The `dispatch_action()` function (`src/dispatch.rs`) handles all action variants. It returns `bool` — `true` means quit.

## Audio Engine

Located in `src/audio/`. Communicates with SuperCollider (scsynth) via OSC over UDP.

### Key Components

- `AudioEngine` (`engine.rs`) — manages synth nodes, bus allocation, routing, voice allocation
- `OscClient` (`osc_client.rs`) — OSC message/bundle sending
- `BusAllocator` (`bus_allocator.rs`) — audio/control bus allocation

### SuperCollider Groups

```
GROUP_SOURCES    = 100  — all source synths (oscillators, samplers)
GROUP_PROCESSING = 200  — filters, effects, mixer processing
GROUP_OUTPUT     = 300  — output synths
GROUP_RECORD     = 400  — recording nodes
```

### Instrument → Synth Mapping

Instruments map to SuperCollider nodes in two ways:
1. **Persistent Nodes:** `AudioIn` and `BusIn` instruments have static source nodes.
2. **Polyphonic Voices:** Oscillator and Sampler instruments spawn a new "voice chain" (group containing source + midi control node) for every note-on event.

Filters and effects are currently static per-instrument nodes (shared by all polyphonic voices), though the architecture allows for per-voice effects in the future.

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
5. Call `AudioEngine::spawn_voice()` for note-ons (sends OSC bundles)
6. Track active notes and call `AudioEngine::release_voice()` when expired

Tick resolution: 480 ticks per beat. Notes are sent as OSC bundles with NTP timetags for sample-accurate scheduling.

## Persistence

SQLite database via `rusqlite`. Implementation in `src/state/persistence.rs`.

### What's Persisted

Comprehensive — the full state survives save/load:
- Instruments (source type, name, filter, LFO, envelope, polyphonic, mixer controls)
- Source parameters (with type: float/int/bool)
- Effects chain (type, params, enabled, ordering)
- Sends (per-instrument bus sends with level and enabled)
- Filter modulation sources (LFO, envelope, or instrument-param cross-modulation)
- Mixer buses (name, level, pan, mute, solo)
- Master level and mute
- Piano roll tracks and notes
- Musical settings (BPM, time signature, key, scale, tuning, loop points)
- Automation lanes and points (with curve types)
- Sampler configs (buffer, loop mode, slices)
- Drum Sequencer state (pads, patterns, steps)
- Chopper state
- MIDI recording settings and mappings
- Custom synthdef registry (name, params, source path)

### What's NOT Persisted

- UI selection state (instrument selection, mixer selection) is partially saved in `session` table
- Playback position
- Audio engine state (rebuilt on connect)
- Audio waveforms (regenerated on load/record)
