# Refactor Plan: Audio Engine & UI Engine

Deep dive analysis of the tuidaw codebase after multiple
iterations. Documents active bugs, iteration artifacts, and
architectural recommendations for both the audio engine and UI layer.

---

## Part 1: Active Bugs

These are things that are broken right now and should be fixed before
any structural refactoring.

### ~~1.1 Automation node index calculation ignores LFO nodes~~ FIXED

Originally quick-fixed by adding `strip.lfo.enabled` checks to
positional index calculations. Now properly fixed by the `StripNodes`
refactor (Part 3) -- `apply_automation()` uses named fields
(`nodes.filter`, `nodes.output`, `nodes.effects`) instead of
positional indexing entirely.

---

### ~~1.2 Filter resonance parameter name mismatch~~ FIXED

Changed `"res"` to `"resonance"` in the FilterResonance automation
handler to match the synthdef parameter name.

---

### 1.3 Sampler automation is a no-op

**File:** `src/audio/engine.rs:1157-1177`

Both `SamplerRate` and `SamplerAmp` automation handlers loop through
voice chains but have empty loop bodies:

```rust
AutomationTarget::SamplerRate(strip_id) => {
    for voice in &self.voice_chains {
        if voice.strip_id == *strip_id {
            // The sampler synth is the second node after MIDI control
            // Note: This is a simplification; ideally we'd track sampler node IDs
            // For now, we update via the MIDI node which won't work directly
            // A proper implementation would need voice-level tracking
        }
    }
}
```

The root cause is that `VoiceChain` only stores `group_id` and
`midi_node_id` -- it doesn't track the source/sampler node ID, so
there's no way to send `/n_set` to the right node.

**Fix:** Add `source_node: i32` to `VoiceChain` (see Architecture
section).

---

### 1.4 `SetStripParam` action is a stub

**File:** `src/dispatch.rs:258-263`

```rust
Action::SetStripParam(strip_id, ref param, value) => {
    let _ = strip_id;
    let _ = param;
    let _ = value;
    // TODO: implement real-time param setting on audio engine
}
```

The action variant exists, panes can emit it, but the handler
explicitly discards all arguments. Any UI that relies on real-time
parameter updates via this action will silently do nothing.

---

## Part 2: Iteration Artifacts

Code that works but is leftover from previous design iterations,
causing confusion or maintenance burden.

### 2.1 Naming mismatch between docs and code

CLAUDE.md references `RackState`, `RackPane`, `ModuleId`, and
describes a "rack" metaphor with "modules" connected by "connections."
The actual code uses `StripState`, `StripPane`, `StripId`, and a
"strip" metaphor with linear signal chains. The old
rack/module/connection abstraction was replaced with channel strips,
but CLAUDE.md wasn't fully updated.

**Affected:** CLAUDE.md, `docs/architecture.md`, various doc comments.

### ~~2.2 `rebuild_routing()` backward-compat alias~~ FIXED

Removed the dead `rebuild_routing()` wrapper method from
`AudioEngine`.

### ~~2.3 Unused `_polyphonic` parameter~~ FIXED

Removed the `_polyphonic: bool` parameter from `spawn_voice()` and
cleaned up all call sites in `dispatch.rs` and `playback.rs`,
including the tuple types and variable extractions that carried the
unused value.

### 2.4 Global dead code suppression

**File:** `src/main.rs:1`

```rust
#![allow(dead_code, unused_imports)]
```

Blanket suppression across the entire crate. Hides genuine unused code
-- there's no way to know what's actually dead without removing this
and checking compiler warnings.

### 2.5 Piano keyboard mapping duplicated 3 times

**Files:**
- `src/panes/piano_roll_pane.rs` -- `key_to_offset_c()` /
  `key_to_offset_a()`
- `src/panes/strip_pane.rs` -- identical functions
- `src/panes/strip_edit_pane.rs` -- identical functions

Three copies of the same QWERTY-to-pitch mapping. If the layout
changes, all three must be updated in sync.

### 2.6 `PushPane`/`PopPane` actions defined but not implemented

**File:** `src/ui/pane.rs:17-20, 214-217`

```rust
// In Action enum:
PushPane(&'static str),
PopPane,

// In PaneManager::handle_input():
Action::PushPane(id) => {
    self.switch_to(id);  // identical to SwitchPane
}
Action::PopPane => {}    // no-op
```

The modal stack concept was planned but never implemented. `PushPane`
just switches (no stack push), `PopPane` does nothing.

### 2.7 `SemanticColor` enum defined but never used

**File:** `src/ui/style.rs:127-157`

A full themed color abstraction (`Text`, `TextMuted`, `Selected`,
`Border`, etc.) with default colors defined. No pane references it --
all use concrete `Color::` constants.

### 2.8 `Keymap::merge()` exists but is never called

**File:** `src/ui/keymap.rs:161-168`

Combines two keymaps with right-precedence. No caller in the codebase.

### 2.9 `SequencerPane` is a placeholder

**File:** `src/panes/sequencer_pane.rs` (54 lines)

Has a single "quit" keybinding and renders "Coming soon..."
text. Registered in main.rs and accessible via the `3` key.

### 2.10 `#[serde(skip)]` annotations on fields that aren't serialized

**Noted in:** `docs/architecture.md:139-143`

`StripState` derives `Serialize`/`Deserialize`, but persistence uses
SQLite (not serde). The `#[serde(skip)]` annotations on `mixer` and
`piano_roll` only exist to prevent compile errors from types that
don't implement `Serialize`. The serde derives themselves are unused.

---

## Part 3: Audio Engine Architecture

### Current Architecture

```
AudioEngine
  client: Option<OscClient>           -- UDP socket to scsynth
  node_map: HashMap<StripId, StripNodes> -- strip -> named node slots
  voice_chains: Vec<VoiceChain>        -- active polyphonic voices
  bus_allocator: BusAllocator          -- audio/control bus allocation
  send_node_map: HashMap<(usize, u8), i32>  -- send synth nodes
  bus_node_map: HashMap<u8, i32>       -- bus output synth nodes
  bus_audio_buses: HashMap<u8, i32>    -- mixer bus SC audio buses
```

### ~~Proposed: Structured node map~~ DONE

Replaced `node_map: HashMap<StripId, Vec<i32>>` with `HashMap<StripId,
StripNodes>`:

```rust
pub struct StripNodes {
    pub source: Option<i32>,      // AudioIn synth (None for oscillator strips)
    pub lfo: Option<i32>,         // LFO modulator
    pub filter: Option<i32>,      // Filter synth
    pub effects: Vec<i32>,        // Effect chain, in order (only enabled effects)
    pub output: i32,              // Output/mixer synth (always exists)
}
```

All methods now use named fields instead of positional
indexing. `rebuild_strip_routing()` builds individual `Option<i32>` /
`Vec<i32>` variables during synth creation, then constructs
`StripNodes` at the end. `apply_automation()` uses `nodes.output`,
`nodes.filter`, and `nodes.effects.get(enabled_idx)` directly. The
effect param automation also now correctly counts only enabled effects
before the target index, fixing a latent bug when disabled effects
preceded the target.

### Proposed: Richer voice tracking

Replace:

```rust
struct VoiceChain {
    strip_id: StripId,
    pitch: u8,
    group_id: i32,
    midi_node_id: i32,
}
```

With:

```rust
struct VoiceChain {
    strip_id: StripId,
    pitch: u8,
    group_id: i32,
    midi_node: i32,
    source_node: i32,    // oscillator or sampler node
    spawn_time: Instant,  // for voice-steal ordering
}
```

Adding `source_node` enables sampler automation (bug 1.3). Adding
`spawn_time` enables proper oldest-voice stealing instead of always
removing the first match in the Vec.

### ~~Proposed: Configurable release cleanup~~ DONE

Replaced the hardcoded 5-second group free with envelope-aware cleanup.
`release_voice()` now takes `&StripState`, looks up the strip's
`amp_envelope.release` time, and schedules cleanup at `offset_secs +
release_time + 1.0`. The +1.0 second margin accounts for
SuperCollider's envelope grain.

### ~~Proposed: Mixer bus allocation through BusAllocator~~ DONE

Replaced hardcoded `bus_audio_base = 200` with
`bus_allocator.get_or_alloc_audio_bus()` calls using sentinel StripIds
(`u32::MAX - bus_id`). Mixer buses now share the allocator's address
space with strip buses, preventing collisions regardless of strip
count.

### ~~Proposed: Stop rebuilding the full graph for mixer changes~~ DONE

Added `update_all_strip_mixer_params(&self, state: &StripState)` which
iterates all strips and sets level/mute/pan on each strip's
`nodes.output` via OSC, without tearing down the graph. Replaced 4
`rebuild_strip_routing()` calls with this method:

- `dispatch.rs` MixerAdjustLevel handler
- `dispatch.rs` MixerToggleMute handler
- `dispatch.rs` MixerToggleSolo handler
- `main.rs` master mute toggle (`.` key)

Updates all strips in each call because master level/mute/solo affect
effective values across all strips. Topology-changing operations
(AddStrip, DeleteStrip, UpdateStrip, ConnectServer, MixerToggleSend)
still use the full `rebuild_strip_routing()`.

---

## Part 4: UI Engine Architecture

### Current Architecture

```
main.rs event loop
  PaneManager
    panes: Vec<Box<dyn Pane>>
    active_index: usize

  StripPane owns StripState (the entire app state)
  Other panes (Mixer, PianoRoll, Add) need StripState for rendering

  Workaround: clone StripState every frame, pass to render_with_state()
```

The central problem: **all application state lives inside a single
pane** (`StripPane`). Other panes need that state for rendering and
input handling, but Rust's borrow checker prevents holding two `&mut`
references into `PaneManager` simultaneously.

The current workaround clones `StripState` every frame for mixer and
piano roll rendering. This also forces the `render_with_state()`
pattern, which requires special-casing in main.rs for every pane that
needs external data.

### Proposed: Extract state from panes

Move `StripState` (and other shared state) out of `StripPane` and into
a top-level `AppState` that lives alongside the pane manager:

```rust
struct AppState {
    strips: StripState,
    session: SessionState,
}

fn run(backend: &mut RatatuiBackend) -> io::Result<()> {
    let mut state = AppState::new();
    let mut panes = PaneManager::new();
    let mut audio_engine = AudioEngine::new();

    loop {
        // Input: panes can read state to decide what action to emit
        let action = panes.handle_input(event, &state);

        // Dispatch: pure state mutation, no pane access needed
        dispatch_action(&action, &mut state, &mut audio_engine);

        // Render: all panes receive state as a read-only reference
        panes.render(&mut frame, &state);
    }
}
```

The `Pane` trait changes to accept `&AppState`:

```rust
trait Pane {
    fn handle_input(&mut self, event: InputEvent, state: &AppState) -> Action;
    fn render(&self, g: &mut dyn Graphics, state: &AppState);
    // ...
}
```

**What this eliminates:**
- Frame-by-frame `StripState` cloning
- All `render_with_state()` / `render_with_registry()` /
  `render_with_full_state()` variants
- All `get_pane_mut::<StripPane>("strip")` downcasting in dispatch.rs
- The special-case render block in main.rs (lines 213-267)
- The `as_any_mut()` requirement on the Pane trait

**What this changes:**
- Every pane's `render()` and `handle_input()` signatures
- `dispatch_action()` takes `&mut AppState` instead of `&mut
  PaneManager`
- Panes that currently own state (`StripPane`, `StripEditPane`,
  `FrameEditPane`) become pure UI views

This is a mechanical refactor -- each pane needs its method signatures
updated, but the logic inside each method stays the same. The dispatch
module becomes simpler because it operates directly on `AppState`
instead of reaching through pane downcasts.

### Proposed: Split the Action enum

The current `Action` enum has 50+ variants mixing UI navigation with
domain operations:

```rust
enum Action {
    None,
    Quit,
    SwitchPane(&'static str),
    PushPane(&'static str),
    PopPane,
    AddStrip(OscType),
    DeleteStrip(StripId),
    // ... 45 more variants
}
```

Split into domain-specific sub-enums:

```rust
enum Action {
    None,
    Quit,
    Nav(NavAction),
    Strip(StripAction),
    Mixer(MixerAction),
    PianoRoll(PianoRollAction),
    Server(ServerAction),
    File(FileAction),
    Session(SessionAction),
}

enum NavAction {
    SwitchPane(&'static str),
    PushPane(&'static str),
    PopPane,
}

enum MixerAction {
    Move(i8),
    Jump(i8),
    AdjustLevel(f32),
    ToggleMute,
    ToggleSolo,
    CycleSection,
    CycleOutput,
    CycleOutputReverse,
    AdjustSend(u8, f32),
    ToggleSend(u8),
}

// etc.
```

Each pane only constructs actions from its own sub-enum. The dispatch
module matches on the outer enum and delegates to focused handler
functions.

### Proposed: Extract piano keyboard utility

Create `src/ui/piano_keyboard.rs`:

```rust
pub struct PianoKeyboard {
    pub octave: i8,
    pub layout: KeyboardLayout,
}

pub enum KeyboardLayout {
    LayoutC,  // z=C, x=D, c=E, ...
    LayoutA,  // a=C, s=D, d=E, ...
}

impl PianoKeyboard {
    pub fn key_to_pitch(&self, key: char) -> Option<u8>;
    pub fn adjust_octave(&mut self, delta: i8);
}
```

Replace the three duplicated `key_to_offset_c()` / `key_to_offset_a()`
implementations in `StripPane`, `StripEditPane`, and `PianoRollPane`
with a shared `PianoKeyboard` instance.

### Proposed: Implement proper pane stack

Replace the current `PushPane`/`PopPane` no-ops with actual stack
behavior in `PaneManager`:

```rust
struct PaneManager {
    panes: Vec<Box<dyn Pane>>,
    active_index: usize,
    stack: Vec<usize>,  // stack of previous active indices
}

// PushPane: save current index, switch to new pane
// PopPane: restore previous index from stack
```

This enables proper modal dialogs (help overlay, file browser,
confirmations) that return to the previous context.

---

## Part 5: Priority Order

### ~~Immediate fixes (bugs)~~ DONE

1. ~~**Fix automation node indexing**~~ -- quick-fixed by adding LFO
   awareness to index calculations
2. ~~**Fix `"res"` -> `"resonance"`**~~ -- corrected param name
3. ~~**Remove `rebuild_routing()` alias**~~ -- removed
4. ~~**Remove `_polyphonic` parameter**~~ -- removed from signature
   and all call sites

### ~~Short-term (audible improvements)~~ DONE

5. ~~**Structured `StripNodes` map**~~ -- replaced `HashMap<StripId,
   Vec<i32>>` with `HashMap<StripId, StripNodes>` using named fields;
   eliminated all positional index calculations
6. ~~**Stop rebuilding the full graph on mixer changes**~~ -- added
   `update_all_strip_mixer_params()` for level/mute/solo/pan; 4
   rebuild calls replaced

### ~~Short-term (remaining audible improvements)~~ DONE

7. ~~**Configurable release cleanup**~~ -- replaced hardcoded 5-second
   group free with `strip.amp_envelope.release + 1.0s` margin;
   `release_voice()` now takes `&StripState` parameter; `playback.rs`
   restructured to hoist state clone for both note-on and note-off
   blocks
8. ~~**Route mixer buses through BusAllocator**~~ -- replaced hardcoded
   `bus_audio_base = 200` formula with `bus_allocator.get_or_alloc_audio_bus()`
   calls using sentinel StripIds (`u32::MAX - bus_id`); mixer buses now
   share the allocator's address space with strip buses, preventing
   collisions

### Next: Medium-term (structural)

9. **Extract `AppState` from panes** -- eliminates clone-per-frame and
   render_with_state pattern
10. **Extract piano keyboard utility** -- deduplicate 3 copies
11. **Add `source_node` to `VoiceChain`** -- enables sampler
    automation
12. **Implement `SetStripParam` action** -- real-time parameter
    updates from UI

### Longer-term (cleanup)

13. **Split Action enum** into sub-enums
14. **Implement pane stack** for proper modals
15. **Remove `#![allow(dead_code)]`** and clean up actual dead code
16. **Update CLAUDE.md** to match current Strip-based naming
17. **Remove unused `SemanticColor`**, `Keymap::merge()`, serde
    derives
18. **Remove or implement `SequencerPane`**
