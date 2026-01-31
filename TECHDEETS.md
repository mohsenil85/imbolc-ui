# Technical Details (TECHDEETS)

This document provides deep technical details on the `ilex` codebase, specifically focusing on the audio engine backend, architecture, and signal flow.

## 1. System Architecture

The application follows a strict Model-View-Update (MVU) inspired architecture, though adapted for a TUI with a dedicated audio thread.

### Core Components
-   **State (`AppState`)**: The single source of truth. Contains `StripState` (tracks/mixer), `SessionState` (global settings), and `InstrumentState`.
-   **UI (`Panes`)**: Stateless views that render based on `AppState`. They emit `Action` enums but do not mutate state directly.
-   **Dispatch (`dispatch.rs`)**: The central event handler. Receives `Action`s, mutates `AppState`, and instructs the `AudioEngine`.
-   **Audio Engine (`AudioEngine`)**: Manages the SuperCollider server (scsynth) and mirrors the abstract `InstrumentState` into a concrete DSP graph.

### Data Flow
1.  **Input**: User presses a key (e.g., Space to play).
2.  **Pane**: `PianoRollPane` handles the key and returns `Action::PianoRoll(PianoRollAction::PlayStop)`.
3.  **Dispatch**: `dispatch_action` matches this action.
    -   Mutates `state.session.piano_roll.playing`.
    -   Calls `audio_engine.release_all_voices()` if stopping.
4.  **Render**: The main loop re-renders the UI with the new state.

## 2. Audio Engine Backend (Rust + SuperCollider)

The audio engine is a hybrid system: logic and scheduling in Rust, DSP in SuperCollider (`scsynth`).

### Rust Side (`src/audio/`)
-   **`AudioEngine`**: The main controller.
    -   Manages the `scsynth` child process.
    -   Holds `OscClient` for UDP communication.
    -   Maintains a `node_map` (Instrument ID -> SC Node IDs) to track the active graph.
    -   Manages `voice_chains` for polyphony.
-   **`BusAllocator`**: Deterministically allocates audio and control buses.
    -   Ensures instruments and buses have dedicated channels.
    -   Resets on project load/engine restart.
-   **Scheduling**:
    -   `playback.rs` runs a tick-based scheduler (~16ms intervals).
    -   Notes are sent as **OSC Bundles** with future timestamps (NTP) for sample-accurate timing, compensating for the UDP network jitter.

### SuperCollider Side (`synthdefs/`)
-   **SynthDefs**: Pre-compiled DSP graphs loaded into the server.
    -   **Sources**: `ilex_saw`, `ilex_sampler`, `ilex_audio_in`.
    -   **Processing**: `ilex_lpf`, `ilex_reverb`, `ilex_tape_comp`.
    -   **Output**: `ilex_output`, `ilex_bus_out`.
-   **Groups**: Execution order is enforced via Groups:
    -   `100` (Sources): Oscillators, Samplers, Audio Input.
    -   `200` (Processing): Filters, Effects.
    -   `300` (Output): Faders, Panners, Bus Outputs.
    -   `400` (Record): Disk recording nodes.

## 3. Signal Flow & Routing

Routing is dynamic but deterministic, rebuilt whenever the instrument structure changes (`rebuild_instrument_routing`).

### Instrument Chain
Each "Instrument" (Strip) is realized as a chain of SuperCollider nodes:

1.  **Source Stage**:
    -   **Polyphonic (Osc/Sampler)**: Voices are spawned dynamically (see below) and sum into a dedicated `source_out` audio bus.
    -   **Monophonic (AudioIn/BusIn)**: A persistent synth (`ilex_audio_in`) runs in Group 100, writing to `source_out`.
2.  **Filter Stage** (Group 200):
    -   Reads from `source_out`.
    -   Runs `ilex_lpf`/`ilex_hpf`.
    -   Writes to `filter_out` bus.
3.  **Effects Chain** (Group 200):
    -   Linear chain of effects (`ilex_delay`, `ilex_reverb`).
    -   Each reads from previous output and writes to next bus.
4.  **Output Stage** (Group 300):
    -   `ilex_output` synth.
    -   Applies Level, Pan, Mute.
    -   Writes to Hardware Output (Bus 0) OR a Mixer Bus.

### Polyphony Implementation
Polyphony is handled by "Voice Chaining":
-   When a note starts, `AudioEngine::spawn_voice` does the following:
    1.  Allocates a new **Group** for the voice.
    2.  Creates a **Control Synth** (`ilex_midi`) in that group. It outputs `freq`, `gate`, `vel` to temporary control buses.
    3.  Creates a **Source Synth** (`ilex_saw`, etc.) in that group. It reads from those control buses.
    4.  The source synth writes to the instrument's shared `source_out` audio bus.
-   **Voice Stealing**: If `MAX_VOICES` (16) is reached, the oldest voice group is freed.
-   **Architecture**: This is a "Paraphonic-ish" hybrid where voices are individual sources, but they all feed into a single, global Filter/FX chain per instrument.

## 4. Modulation System

Modulation is baked into the SynthDefs using a `Select.kr` pattern.

-   **Params**: Every modulatable parameter (e.g., `cutoff`) has a corresponding `_in` parameter (e.g., `cutoff_mod_in`).
-   **Logic**:
    ```supercollider
    // logic inside SynthDef
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod));
    ```
-   **Routing**:
    -   LFOs are realized as `ilex_lfo` synths running in Group 100.
    -   They output to a control bus.
    -   The target synth (e.g., Filter) is configured with `cutoff_mod_in` pointing to that control bus index.
    -   If no modulation is active, `-1` is passed, and the synth uses the static `cutoff` value.

## 5. Persistence

-   **Database**: SQLite (`rusqlite`).
-   **File**: `~/.config/ilex/default.sqlite`.
-   **Schema**:
    -   `instruments`: Core strip configuration (name, type, params).
    -   `effects`: Chain of effects per instrument.
    -   `session`: Global settings (BPM, buses).
    -   `notes`: Piano roll data.
    -   `automation`: Automation points.
-   **Philosophy**: The database saves the *Project Model*, not the *Audio Graph*. The Audio Graph is reconstructed from the Project Model on load.

## 6. Key Dispatch Functions

-   `dispatch_action` (src/dispatch.rs): Main entry point.
-   `rebuild_instrument_routing` (src/audio/engine.rs): The "commit" function that syncs audio graph to state.
-   `spawn_voice` (src/audio/engine.rs): Handles Note-On logic.
-   `playback::tick` (src/playback.rs): The sequencer clock.

## 7. Known Limitations / Future Work

-   **Latency**: TUI rendering and input handling are on the main thread; heavy UI loads could theoretically jitter the playback *scheduling* (though OSC bundles mitigate this).
-   **Voice Stealing**: Currently strict FIFO. Could be smarter (quietest first).
-   **Parameter Smoothing**: Rapid UI slider movement sends discrete OSC messages. Some SynthDefs use `Lag.kr` (implied in `EnvGen`) but zippering is possible on raw params.
