# imbolc-net: Networked Jam Space

## Problem

Multiple musicians in the same physical space want to jam together using imbolc. Each person has their own screen and controller. Audio inputs (guitars, mics) are cabled to a central machine. MIDI controllers connect via RTP-MIDI over local ethernet (discovered by the OS, not us). There is a single master audio output. We never send audio over the network — only control data.

## Architecture

### Crate Structure

```
imbolc-types    — All shared types: AppState, Action, Instrument, etc.
                  No logic, just data structures + serde derives

imbolc-core     — Dispatch logic, audio engine, SuperCollider communication
                  Depends on: imbolc-types

imbolc-net      — Network layer: RemoteDispatcher (client), NetServer (server)
                  Depends on: imbolc-types (NOT imbolc-core)

imbolc          — TUI binary
                  Depends on: imbolc-types, imbolc-core (local), imbolc-net (remote)
```

Extracting `imbolc-types` keeps the client lightweight — it only needs the data structures for serialization, not the dispatch/audio code.

**Dependency graph:**

```
                    imbolc-types
                    /     |     \
                   /      |      \
                  v       v       v
          imbolc-core  imbolc-net  imbolc (binary)
                  \       /         /
                   \     /         /
                    v   v         v
               [server binary]  [client binary]
```

Note: `imbolc-net` does NOT depend on `imbolc-core`. They're siblings that share `imbolc-types`.

### The Dispatch Seam

The network boundary lives at the dispatch layer. The TUI doesn't know or care whether dispatch is local or remote.

```rust
trait Dispatcher {
    fn dispatch(&mut self, action: Action) -> DispatchResult;
    fn state(&self) -> &AppState;
}
```

Two implementations:

- **LocalDispatcher** — calls `imbolc-core::dispatch()` directly, owns the state and audio engine
- **RemoteDispatcher** — serializes the action, sends to server, receives state updates

The binary picks which one at startup. Pane code never changes.

### Networked Mode

```
┌─────────────────────────┐              ┌─────────────────────────────────┐
│  Client machine         │              │  Server machine                 │
│                         │     LAN      │                                 │
│  imbolc (TUI)           │              │  imbolc-net (NetServer)         │
│  imbolc-net (Remote-    │  ─Action──>  │  imbolc-core (LocalDispatcher)  │
│    Dispatcher)          │  <──State──  │  SuperCollider                  │
│  imbolc-types           │              │  imbolc-types                   │
│                         │              │  Audio I/O (all of it)          │
│  No SC, no audio        │              │                                 │
└─────────────────────────┘              └─────────────────────────────────┘
         x N clients                                1 server
```

### Local Mode

When running solo, `imbolc-net` is not used. The binary instantiates `LocalDispatcher` directly.

```
imbolc (TUI) -> LocalDispatcher (imbolc-core) -> SuperCollider
```

The binary detects which mode at startup (flag, config, or presence of server).

### Binaries

One binary, multiple modes:

```
imbolc                     # local mode (default, same as today)
imbolc --server            # server mode: headless, runs NetServer + LocalDispatcher + SC
imbolc --server --tui      # server mode with TUI (host is also playing)
imbolc --connect <addr>    # client mode: TUI + RemoteDispatcher, no SC
```

Alternatively, separate binaries (`imbolc` and `imbolc-server`), but flags are simpler to start.

## What `imbolc-net` Does

A thin crate with two components. Depends only on `imbolc-types`, not `imbolc-core`.

### RemoteDispatcher (client component)

Implements the `Dispatcher` trait for network mode:

- `dispatch()` serializes the `Action` and sends it to the server
- `state()` returns the cached `AppState` received from the server
- Maintains TCP connection to server
- Background thread receives state updates, swaps the cached state

The TUI calls the same `Dispatcher` interface whether local or remote — it has no awareness of the network.

### NetServer (server component)

Runs on the server machine alongside `imbolc-core`:

- Listens for client connections (TCP)
- Receives `Action` messages from clients
- Validates ownership (is this client allowed to do this?)
- Forwards valid actions to the `LocalDispatcher`
- After dispatch, broadcasts the new `AppState` to all connected clients
- Manages connection lifecycle (join, disconnect, reconnect)

The server binary might be headless (no TUI) or have a TUI for the host who's also playing.

### What It Does NOT Do

- Audio transport — all audio is local to the server
- MIDI transport — RTP-MIDI handles this at the OS layer
- Complex conflict resolution — server is authoritative, last write wins
- Depend on `imbolc-core` — only needs types for serialization

## State Model

**Full mirror.** Every client holds a complete copy of `AppState`. The server broadcasts the full state (or diffs — optimization for later). Clients render the same UI as local mode.

This is simpler to build, and visibility restrictions can be added in the UI layer later without changing the protocol.

## Ownership

Each connected client owns one or more instruments. Ownership determines which actions the server will accept from a given client.

- A client can only mutate state on instruments it owns
- Transport controls (play, stop, BPM) — TBD, may require a privileged node
- Piano roll edits — scoped to owned instruments' tracks
- Mixer — TBD (own channel only? or global?)

Ownership is assigned on connect. Mechanism TBD (server assigns, client requests, configured in advance).

## Protocol

LAN only. Control data is small. Latency budget is generous for non-audio data on a local network (sub-millisecond typical).

- **Transport:** TCP for reliability. Messages are small and infrequent enough that TCP's overhead doesn't matter. UDP adds complexity for no real gain at this scale.
- **Serialization:** serde — `Action` and `AppState` already derive or can derive `Serialize`/`Deserialize`. Wire format TBD (bincode for compactness, or MessagePack, or JSON for debuggability during development).
- **Message types:**
  - Client -> Server: `Action` (already the unit of intent)
  - Server -> Client: `AppState` snapshot (initially), then possibly diffs

## Discovery

TBD. Options:

- **mDNS/Bonjour** — zero-config, appropriate for LAN
- **Manual IP** — simple, always works
- **Both** — mDNS with manual fallback

Not a priority for v1. Manual IP is fine to start.

## Monitoring

The server machine has the audio hardware. Players need to hear themselves and the mix. Options (not mutually exclusive):

- Dedicated hardware outputs per player (server needs a multi-output interface)
- Cue bus system within SuperCollider (per-player headphone mixes)
- Single shared monitor output (simplest, maybe fine for jamming)

This is a hardware/SC routing question more than an `imbolc-net` question. Defer until we can try things.

## Deferred Decisions

These are intentionally left open. They'll be resolved by feel once the basic system is running.

| Question | Options | Notes |
|----------|---------|-------|
| Ownership granularity | Per-instrument, per-track, per-set | Start with per-instrument |
| Privileged node | One host with extra powers vs. all equal | Leaning toward one privileged node for transport/save/load |
| Global read-only scope | See everything, see piano roll only, see nothing | Start with full visibility |
| Monitoring | Hardware outs, cue buses, shared | Hardware dependent |
| Save/load authority | Server only, privileged client, any client | Server only is safest default |
| Reconnection | Rejoin with same ownership, reassign, manual | Needs to feel right |
| Wire format | bincode, MessagePack, JSON | JSON for dev, compact format for later |
| Discovery | mDNS, manual, both | Manual first |

## Implementation Sketch

### Phase 0: Extract Types

Create `imbolc-types` crate with all shared data structures. Most types in imbolc-core are already pure data — this is largely a mechanical move.

**Definitely moves (pure data, no dependencies):**

From `action.rs` (~100% of file):
- `Action`, `DispatchResult`, `AudioDirty`
- All sub-action enums: `InstrumentAction`, `MixerAction`, `PianoRollAction`, `SequencerAction`, `AutomationAction`, `SessionAction`, `ArrangementAction`, `NavAction`, etc.
- `VstTarget`, `VstParamAction`, `FilterParamKind`, `LfoParamKind`
- `ToggleResult`, `FileSelectAction`, `NavIntent`, `StatusEvent`

From `state/param.rs`:
- `Param`, `ParamValue`

From `state/instrument/`:
- `Instrument`, `InstrumentId`, `SourceType`, `EffectType`, `EffectSlot`, `EffectId`
- `FilterType`, `FilterConfig`, `EqBandType`, `EqBand`, `EqConfig`
- `LfoConfig`, `EnvConfig`, `ModulatedParam`, `ModSource`, `InstrumentSection`
- `OutputTarget`, `MixerSend`, `MixerBus`

From `state/piano_roll.rs`:
- `Note`, `Track`, `PianoRollState`

From `state/automation/`:
- `AutomationLaneId`, `CurveType`, `AutomationPoint`

From `state/arrangement.rs`:
- `ClipId`, `PlacementId`, `PlayMode`, `Clip`, `ClipPlacement`, `ClipEditContext`, `ArrangementState`

From `state/session.rs`:
- `MixerSelection`, `MusicalSettings`

From `state/vst_plugin.rs`:
- `VstPluginId`, `VstPluginKind`, `VstParamSpec`, `VstPlugin`

From `state/custom_synthdef.rs`:
- `CustomSynthDefId`, `ParamSpec`, `CustomSynthDef`

From `state/mod.rs`:
- `PendingRender`, `PendingExport`, `KeyboardLayout`, `VisualizationState`, `IoGeneration`

From `audio/engine/`:
- `ServerStatus` (simple enum, only external dep in state types)

**Needs consideration:**

These types have methods that orchestrate other types. The struct definitions are pure data, but they have impl blocks with business logic:

- `AppState` — top-level state, has `ServerStatus` dependency
- `SessionState` — contains registries, has utility methods
- `InstrumentState` — instrument collection management
- `AutomationState` — lane management
- `CustomSynthDefRegistry`, `VstPluginRegistry` — lookup logic
- `Clipboard` — likely pure, just needs verification

**Strategy:** Move the struct/enum definitions to `imbolc-types`. Keep impl blocks with complex logic in `imbolc-core` (Rust allows impl blocks in a different crate than the type definition, as long as they don't impl foreign traits). Simple accessor methods can move with the types.

**Tasks:**
1. Create `imbolc-types` crate at `../imbolc-types/`
2. Move type definitions (structs, enums, type aliases)
3. Add `Serialize`/`Deserialize` derives to everything
4. `imbolc-core` depends on `imbolc-types`, re-exports for backwards compatibility
5. Move simple impl blocks (accessors, pure helpers) with the types
6. Keep complex impl blocks (state management, registry lookups) in `imbolc-core`
7. Verify existing code still compiles

This is the biggest mechanical change. Everything else is additive.

### Phase 1: Dispatcher Trait

- Define `Dispatcher` trait in `imbolc-types` (or a shared location)
- Create `LocalDispatcher` wrapping existing dispatch logic
- Update `imbolc` binary to use `Dispatcher` trait instead of calling dispatch directly
- Verify local mode still works identically

### Phase 2: Network Plumbing

- Create `imbolc-net` crate (depends on `imbolc-types` only)
- Implement `RemoteDispatcher`: connect, send actions, receive state
- Implement `NetServer`: listen, receive actions, broadcast state
- Define wire protocol: `NetMessage` enum (Action, StateUpdate, Connect, Disconnect, etc.)
- Binary flags: `--server` / `--connect <addr>`
- Get basic round-trip working: client sends action, server dispatches, client sees updated state

### Phase 3: Ownership

- Client identifies itself on connect (name, requested instruments)
- Server tracks ownership table
- Server rejects actions that violate ownership
- UI indicates which instruments are owned by whom

### Phase 4: Polish

- Reconnection handling
- Discovery (mDNS)
- State diffing instead of full broadcasts
- Monitoring / cue bus routing
- Privileged node semantics
