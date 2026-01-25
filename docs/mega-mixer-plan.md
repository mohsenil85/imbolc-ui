# Mega Mixer Implementation Plan

## Overview

Integrate OUTPUT modules with a mega mixer system:
- Each OUTPUT module creates a channel in the mixer (up to 128)
- 64 submix buses for grouping
- 1 master output
- New MIXER view for visual mixing

**Signal flow:** Oscillators/sources → patch to OUTPUT modules → mixer channels → buses/master

The OUTPUT module becomes the explicit "send to mixer" point. Users control how many mixer channels exist by creating OUTPUT modules. This gives explicit routing control rather than auto-assigning every audio source.

## Phase 1: State Layer Foundation

### Task 1.1: Create MixerChannel record
**Sonnet-capable: Yes**

Create `src/main/java/com/tuidaw/state/MixerChannel.java`:
```java
public record MixerChannel(
    int id,                    // 1-128
    String moduleId,           // which module is assigned here (nullable)
    double level,              // 0.0-1.0, default 0.8
    double pan,                // -1.0 to 1.0, default 0.0
    boolean mute,
    boolean solo,
    OutputMode outputMode,     // STEREO, MONO, LEFT, RIGHT
    OutputTarget outputTarget  // MASTER or BUS_1..BUS_64
) {
    public enum OutputMode { STEREO, MONO, LEFT, RIGHT }
    public enum OutputTarget {
        MASTER,
        BUS_1, BUS_2, /* ... */ BUS_64;

        public static OutputTarget bus(int n) { /* ... */ }
    }

    public static MixerChannel empty(int id) { /* defaults */ }
    public MixerChannel withLevel(double level) { /* ... */ }
    // ... other with* methods
}
```

**Acceptance:** Record compiles, has all with* methods, sensible defaults.

---

### Task 1.2: Create MixerBus record
**Sonnet-capable: Yes**

Create `src/main/java/com/tuidaw/state/MixerBus.java`:
```java
public record MixerBus(
    int id,                    // 1-64
    String name,               // user-assignable, default "Bus 1"
    double level,              // 0.0-1.0, default 0.8
    double pan,                // -1.0 to 1.0, default 0.0
    boolean mute,
    boolean solo
) {
    public static MixerBus create(int id) { /* defaults */ }
    // ... with* methods
}
```

**Acceptance:** Record compiles, has all with* methods.

---

### Task 1.3: Create MixerState record
**Sonnet-capable: Yes**

Create `src/main/java/com/tuidaw/state/MixerState.java`:
```java
public record MixerState(
    Map<Integer, MixerChannel> channels,  // 1-128
    Map<Integer, MixerBus> buses,         // 1-64
    double masterLevel,
    double masterPan,
    int selectedChannel,                   // for UI navigation
    int selectedBus                        // -1 if on channels, 1-64 if on bus
) {
    public static final int MAX_CHANNELS = 128;
    public static final int MAX_BUSES = 64;

    public static MixerState initial() {
        // Create empty channels 1-128, buses 1-64
    }

    public MixerChannel getChannel(int id) { /* ... */ }
    public MixerBus getBus(int id) { /* ... */ }
    public Optional<Integer> findFreeChannel() { /* first channel with null moduleId */ }
    // ... with* methods
}
```

**Acceptance:** Record compiles, initial() creates proper structure, findFreeChannel works.

---

### Task 1.4: Add MixerState to RackState
**Sonnet-capable: Yes**

Modify `src/main/java/com/tuidaw/state/RackState.java`:
- Add `MixerState mixer` field to record
- Add `withMixer(MixerState)` method
- Update `initial()` to include `MixerState.initial()`
- Add convenience methods: `getMixerChannel(int)`, `getMixerBus(int)`

**Acceptance:** RackState compiles, includes mixer, tests pass.

---

### Task 1.5: Add mixer StateTransitions
**Sonnet-capable: Yes**

Add to `src/main/java/com/tuidaw/state/StateTransitions.java`:
```java
// Channel operations
public static RackState setChannelLevel(RackState state, int channelId, double level)
public static RackState setChannelPan(RackState state, int channelId, double pan)
public static RackState toggleChannelMute(RackState state, int channelId)
public static RackState toggleChannelSolo(RackState state, int channelId)
public static RackState setChannelOutputMode(RackState state, int channelId, OutputMode mode)
public static RackState setChannelOutputTarget(RackState state, int channelId, OutputTarget target)
public static RackState assignModuleToChannel(RackState state, String moduleId, int channelId)

// Bus operations
public static RackState setBusLevel(RackState state, int busId, double level)
public static RackState setBusPan(RackState state, int busId, double pan)
public static RackState toggleBusMute(RackState state, int busId)
public static RackState toggleBusSolo(RackState state, int busId)
public static RackState setBusName(RackState state, int busId, String name)

// Master operations
public static RackState setMasterLevel(RackState state, double level)
public static RackState setMasterPan(RackState state, double pan)

// Navigation
public static RackState mixerSelectChannel(RackState state, int channelId)
public static RackState mixerSelectBus(RackState state, int busId)
public static RackState mixerMoveSelection(RackState state, int delta)
```

**Acceptance:** All methods compile, pure functions, return new state.

---

## Phase 2: Auto-Assignment Logic

### Task 2.1: Add mixerChannel field to Module
**Sonnet-capable: Yes**

Modify `src/main/java/com/tuidaw/state/Module.java`:
- Add `Integer mixerChannel` field (nullable - null means not mixer-routed)
- Add `withMixerChannel(Integer)` method
- Update `create()` methods to accept optional mixer channel

**Acceptance:** Module compiles, can store mixer channel assignment.

---

### Task 2.2: Create MixerAssigner utility
**Sonnet-capable: Yes**

Create `src/main/java/com/tuidaw/audio/MixerAssigner.java`:
```java
public class MixerAssigner {
    /**
     * Determines if a module type should be auto-assigned to a mixer channel.
     * Only OUTPUT modules get mixer channels - they are the explicit "send to mixer" point.
     * Audio sources (oscillators, samplers) patch to OUTPUT modules, which then appear in mixer.
     */
    public static boolean shouldAutoAssign(ModuleType type) {
        return type == ModuleType.OUTPUT;  // Only OUTPUT modules create mixer channels
    }

    /**
     * Find next available mixer channel.
     */
    public static Optional<Integer> findFreeChannel(MixerState mixer) {
        for (int i = 1; i <= MixerState.MAX_CHANNELS; i++) {
            if (mixer.getChannel(i).moduleId() == null) {
                return Optional.of(i);
            }
        }
        return Optional.empty();
    }
}
```

**Acceptance:** OUTPUT modules get mixer channels, other modules do not.

---

### Task 2.3: Integrate auto-assignment into Rack.addModule
**Sonnet-capable: Yes (with guidance)**

Modify `src/main/java/com/tuidaw/audio/Rack.java` `addModule()` method:
1. After creating module, check `MixerAssigner.shouldAutoAssign(type)`
2. If true, call `MixerAssigner.findFreeChannel(state.mixer())`
3. If channel found:
   - Update module with `withMixerChannel(channelId)`
   - Update mixer state with `assignModuleToChannel(state, moduleId, channelId)`
4. Set state with both updates

**Acceptance:** Creating an OUTPUT module auto-assigns to channel 1, creating a saw-osc does not.

---

### Task 2.4: Handle module deletion - free mixer channel
**Sonnet-capable: Yes**

Modify `StateTransitions.removeModule()`:
1. Check if module has mixer channel assigned
2. If so, clear that channel's moduleId in mixer state
3. Return state with both module removed and channel freed

**Acceptance:** Deleting a module frees its mixer channel for reuse.

---

## Phase 3: View Layer

### Task 3.1: Add MIXER to View enum
**Sonnet-capable: Yes**

Modify `src/main/java/com/tuidaw/core/View.java`:
- Add `MIXER` to enum

**Acceptance:** Compiles.

---

### Task 3.2: Create MixerViewRenderer
**Sonnet-capable: Partially** (complex layout logic)

Create `src/main/java/com/tuidaw/tui/render/MixerViewRenderer.java`:

```
╭─ MIXER ─────────────────────────────────────────────────────────────────────╮
│  CH1      CH2      CH3      CH4      CH5    ║  BUS1    BUS2   ║  MASTER    │
│ saw-1    saw-2    lpf-1     ---      ---    ║ Drums   Synths  ║            │
│ ▮▮▮▮▮▯▯  ▮▮▮▯▯▯▯  ▮▮▮▮▯▯▯  ▯▯▯▯▯▯▯  ▯▯▯▯▯▯▯ ║ ▮▮▮▮▯▯  ▮▮▮▯▯▯  ║  ▮▮▮▮▮▮▯   │
│  -3dB    -12dB     -6dB     -∞       -∞     ║  -6dB   -9dB    ║    0dB     │
│  ◀──●     ●──▶      ●        ●        ●     ║   ●       ●     ║     ●      │
│   M S     M S      M S      M S      M S    ║  M S     M S    ║            │
│ [ST>MST] [L>B1]  [MO>MST]  [ST>MST] [ST>MST]║ ────────────────║            │
├─────────────────────────────────────────────────────────────────────────────┤
│ [←/→] Select  [↑/↓] Level  [m] Mute  [s] Solo  [o] Output  [TAB] Ch/Bus    │
╰─────────────────────────────────────────────────────────────────────────────╯
```

Features:
- Show channels with assigned modules (scroll if > visible)
- Level meters (visual bars)
- dB readout
- Pan indicator
- Mute/Solo indicators
- Output routing indicator (ST=stereo, MO=mono, L/R, >MST=master, >B1=bus1)
- Selected channel highlighted

**Guidance needed:** Layout calculations, scrolling logic, meter rendering.

**Acceptance:** Renders mixer state, shows channels/buses/master, highlights selection.

---

### Task 3.3: Create MixerViewDispatcher
**Sonnet-capable: Yes**

Create `src/main/java/com/tuidaw/core/dispatchers/MixerViewDispatcher.java`:

Handle actions:
- `MOVE_LEFT/RIGHT` - select prev/next channel or bus
- `MOVE_UP/DOWN` or `PARAM_INC/DEC` - adjust level of selected
- `CONFIRM` - cycle output mode (Stereo→Mono→L→R)
- Custom actions needed:
  - Toggle mute (m key)
  - Toggle solo (s key)
  - Cycle output target (o key)
  - Switch between channels/buses section (TAB)
- `CANCEL` - return to rack view

**Acceptance:** All navigation and adjustments work, state updates correctly.

---

### Task 3.4: Add mixer keybindings
**Sonnet-capable: Yes**

Modify all binding files (`VimBinding.java`, `NormieBinding.java`, `EmacsBinding.java`):
- Add `MIXER_VIEW` action mapping (e.g., 'M' in vim, F6 in normie)
- Add `TOGGLE_MUTE`, `TOGGLE_SOLO`, `CYCLE_OUTPUT` actions

Modify `Action.java`:
- Add new actions: `MIXER_VIEW`, `TOGGLE_MUTE`, `TOGGLE_SOLO`, `CYCLE_OUTPUT`, `CYCLE_OUTPUT_TARGET`

**Acceptance:** Can enter mixer view, use all mixer-specific keys.

---

### Task 3.5: Register MixerViewDispatcher and MixerViewRenderer
**Sonnet-capable: Yes**

Modify `Dispatcher.java`:
- Add MixerViewDispatcher to viewDispatchers map

Modify `Renderer.java`:
- Add MixerViewRenderer
- Add case for View.MIXER

Modify `RackViewDispatcher.java`:
- Handle MIXER_VIEW action to enter mixer

**Acceptance:** Can navigate to mixer view and back.

---

### Task 3.6: Update help text for mixer
**Sonnet-capable: Yes**

Update `RackViewRenderer.java` help text to show mixer shortcut.
Update `HelpViewRenderer.java` with mixer documentation.

**Acceptance:** Help shows mixer keybinding.

---

## Phase 4: Patch Integration

### Task 4.1: Expose mixer channels as patch ports
**Sonnet-capable: Partially** (needs architectural understanding)

Modify `StateTransitions.getAllPorts()`:
- In addition to module ports, include mixer channel outputs
- Format: `Port("mix", "ch1")`, `Port("mix", "bus-1")`, `Port("mix", "master")`

These represent the post-fader signal from each mixer channel.

**Acceptance:** Patch view shows mixer channels as routable sources.

---

### Task 4.2: Update PatchViewRenderer for mixer ports
**Sonnet-capable: Yes**

Modify rendering to visually distinguish mixer ports:
- Color code or prefix mixer ports
- Show as `[MIX] ch1 (saw-1)` including assigned module name

**Acceptance:** Mixer ports visible and identifiable in patch view.

---

## Phase 5: Audio Layer (SuperCollider)

### Task 5.1: Design mixer SynthDef
**Sonnet-capable: No** (requires SuperCollider expertise)

Create SuperCollider SynthDef for mixer channel:
```supercollider
SynthDef(\mixerChannel, {
    arg inBus, outBus, level=0.8, pan=0, mute=0, outputMode=0;
    var sig = In.ar(inBus, 2);
    sig = sig * level * (1 - mute);
    sig = Select.ar(outputMode, [
        sig,                           // 0: stereo passthrough
        Pan2.ar(Mix.ar(sig), pan),     // 1: mono with pan
        [sig[0], Silent.ar],           // 2: left only
        [Silent.ar, sig[1]]            // 3: right only
    ]);
    Out.ar(outBus, sig);
}).add;
```

**Needs:** SC expertise to design efficient mixer routing.

---

### Task 5.2: Allocate mixer buses in AudioEngine
**Sonnet-capable: Partially**

Modify `BusAllocator.java`:
- Reserve bus ranges for mixer channels (128 stereo buses)
- Reserve bus range for mixer buses (64 stereo buses)
- Reserve master bus

**Acceptance:** Buses allocated, no conflicts with module buses.

---

### Task 5.3: Create mixer synth nodes on startup
**Sonnet-capable: Partially**

Modify `Rack.java`:
- On audio engine connect, create mixer channel synths
- Create bus summing synths
- Create master output synth
- Wire routing based on channel output targets

**Acceptance:** Audio flows through mixer to output.

---

### Task 5.4: Handle mixer parameter changes
**Sonnet-capable: Yes**

Add effect handlers in `Rack.java`:
```java
case EffectRequest.SetMixerChannelLevel(int ch, double level) -> { /* nodeSet */ }
case EffectRequest.SetMixerChannelPan(int ch, double pan) -> { /* nodeSet */ }
case EffectRequest.SetMixerChannelMute(int ch, boolean mute) -> { /* nodeSet */ }
// etc.
```

Add corresponding EffectRequest types in `Dispatcher.java`.

**Acceptance:** Mixer changes in UI affect audio.

---

### Task 5.5: Route OUTPUT modules through mixer
**Sonnet-capable: Partially**

Modify `Rack.addModule()`:
- When OUTPUT module is created and assigned to mixer channel, route its audio output through that channel
- OUTPUT module's input bus receives patched audio, output goes to mixer channel

Signal flow: `source.out → OUTPUT.in → mixer channel → bus/master`

**Acceptance:** Audio patched to OUTPUT modules flows through mixer, can adjust level/pan in MIXER view.

---

## Phase 6: Cleanup & Polish

### Task 6.1: Enhance OUTPUT module display
**Sonnet-capable: Yes**

- In RACK view, show mixer channel number next to OUTPUT modules (e.g., "out-1 [CH1]")
- In MIXER view, show OUTPUT module name in channel strip
- Consider visual indicator when OUTPUT is muted/soloed in mixer

**Acceptance:** Clear visual link between OUTPUT modules and their mixer channels.

---

### Task 6.2: Add mixer state to save/load
**Sonnet-capable: Yes** (once save/load exists)

Ensure mixer state is serialized/deserialized with rack state.

**Acceptance:** Mixer settings persist across sessions.

---

### Task 6.3: Solo logic implementation
**Sonnet-capable: Yes**

Implement solo behavior:
- When any channel is soloed, mute all non-soloed channels
- Multiple channels can be soloed (solo-in-place)
- Buses can be soloed too

This is pure state logic in StateTransitions.

**Acceptance:** Solo works correctly, multiple solos work, unsolo restores.

---

## Task Dependency Graph

```
Phase 1 (State):
  1.1 ─┬─▶ 1.3 ──▶ 1.4 ──▶ 1.5
  1.2 ─┘

Phase 2 (Auto-assign):
  1.4 ──▶ 2.1 ──▶ 2.2 ──▶ 2.3 ──▶ 2.4

Phase 3 (View):
  1.5 ──▶ 3.1 ──▶ 3.2 ──┬──▶ 3.5 ──▶ 3.6
                3.3 ──┤
                3.4 ──┘

Phase 4 (Patch):
  1.5, 3.5 ──▶ 4.1 ──▶ 4.2

Phase 5 (Audio):
  5.1 ──▶ 5.2 ──▶ 5.3 ──▶ 5.4 ──▶ 5.5

Phase 6 (Cleanup):
  All above ──▶ 6.1 ──▶ 6.2 ──▶ 6.3
```

## Sonnet Feasibility Summary

| Task | Sonnet? | Notes |
|------|---------|-------|
| 1.1-1.5 | Yes | Pure Java records and methods |
| 2.1-2.4 | Yes | Straightforward logic |
| 3.1 | Yes | Trivial |
| 3.2 | Partial | Complex TUI layout, needs detailed spec |
| 3.3-3.6 | Yes | Follow existing patterns |
| 4.1 | Partial | Needs architectural context |
| 4.2 | Yes | Simple rendering change |
| 5.1 | No | Requires SuperCollider expertise |
| 5.2-5.3 | Partial | Needs audio architecture understanding |
| 5.4-5.5 | Yes | Follow existing effect handler pattern |
| 6.1-6.3 | Yes | Cleanup tasks |

## Estimated Effort

- **Phase 1:** 2-3 tasks/session, ~2 sessions
- **Phase 2:** 4 tasks, ~1 session
- **Phase 3:** 6 tasks, ~2-3 sessions (renderer is complex)
- **Phase 4:** 2 tasks, ~1 session
- **Phase 5:** 5 tasks, ~2-3 sessions (audio is tricky)
- **Phase 6:** 3 tasks, ~1 session

**Total:** ~10-12 focused sessions
