# Imbolc Architecture Deep Dive

This document details the internal architecture of Imbolc, focusing on the low-latency audio engine, concurrency model, and integration with SuperCollider.

## High-Level Overview

Imbolc follows a strict separation of concerns between the **UI (Main Thread)** and the **Audio Engine (Audio Thread)**.

*   **Main Thread:** Runs the TUI (Ratatui), handles user input, manages the "source of truth" `AppState`, and renders the interface.
*   **Audio Thread:** A dedicated thread that runs the sequencer clock, manages the SuperCollider server (scsynth), and handles all real-time audio logic.
*   **SuperCollider (scsynth):** Runs as a subprocess. Performs the actual DSP (Digital Signal Processing). Communications happen via Open Sound Control (OSC) over UDP.

```mermaid
graph TD
    User[User Input] -->|Events| Main[Main Thread (TUI)]
    Main -->|AudioCmd (MPSC)| Audio[Audio Thread]
    Audio -->|AudioFeedback (MPSC)| Main
    Audio -->|OSC Bundles (UDP)| SC[SuperCollider (scsynth)]
    SC -->|OSC Reply (UDP)| Audio
    SC -->|Audio Out| Speakers
```

## Threading Model & Audio Processing

The core of Imbolc's timing stability lies in the `AudioThread` loop (`imbolc-core/src/audio/handle.rs` and `audio_thread.rs`).

### 1. The Audio Thread Loop
The audio thread does not rely on the UI framerate. It runs a tight loop that:
1.  **Drains Commands:** Processes pending `AudioCmd`s from the main thread (non-blocking).
2.  **Checks Time:** Calculates the precise elapsed `Duration` since the last tick using `std::time::Instant`.
3.  **Ticks:** If sufficient time has passed (>= 1ms), it calls `tick()`, converting the elapsed duration into **musical ticks** based on the current BPM.
4.  **Yields:** Sleeps for a short duration (`~1ms`) to yield CPU resources to the OS.

### 2. Decoupled Playback Logic
Playback logic is decoupled from wall-clock time. The sequencer advances the playhead by `tick_delta` (derived from the actual elapsed time). This "catch-up" approach ensures that even if the thread sleeps slightly longer than expected (jitter), the playhead stays mathematically correct over time, preventing long-term drift.

### 3. Low Latency & Jitter Compensation (The "Schedule Ahead" Pattern)
To prevent audible jitter caused by the 1ms sleep interval, OS scheduling, or garbage collection (if we were using a GC language), Imbolc uses **OSC Bundles with Timestamps**. This is the key to its tight timing.

When a note is triggered:
1.  The sequencer determines the note starts at `tick X`.
2.  It calculates the exact offset in seconds from "now" (`ticks_from_now * secs_per_tick`).
3.  It calls `osc_time_from_now(offset)` (`imbolc-core/src/audio/osc_client.rs`), which computes an absolute NTP timestamp (UTC).
4.  This timestamp is attached to the OSC bundle sent to SuperCollider.

**The Result:** SuperCollider receives the message *before* the sound needs to play and schedules it for the *exact* sample frame requested. This yields sample-accurate timing independent of Rust thread jitter or network stack latency (as long as the latency is less than the schedule-ahead window).

```rust
// imbolc-core/src/audio/osc_client.rs
pub fn osc_time_from_now(offset_secs: f64) -> OscTime {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)...;
    // Add offset and convert to NTP (1900 epoch)
    ...
}
```

## Concurrency & State Management

State sharing is minimized to prevent locking on the critical path.

### Command / Feedback Pattern
*   **Main -> Audio:** `Sender<AudioCmd>`. Commands like `SpawnVoice`, `UpdateState`, `SetBpm`.
*   **Shadow State:** The audio thread maintains its own "shadow copy" of relevant state (`InstrumentSnapshot`, `SessionSnapshot`, `PianoRollSnapshot`, `AutomationSnapshot`). These are updated via `AudioCmd::UpdateState`. This avoids `Mutex` contention on complex state objects during audio processing.
*   **Audio -> Main:** `Sender<AudioFeedback>`. Events like `PlayheadPosition`, `ServerStatus`, `RecordingState`. The main thread drains this queue every frame to update the UI.

### Shared Monitoring State (The Exception)
For high-frequency visual data (meters, waveforms) where occasional dropped frames are acceptable but blocking the audio thread is not:
*   `AudioMonitor` holds `Arc<Mutex<...>>` for meters, spectrum data, and oscilloscope buffers.
*   This is the *only* shared mutex state. It is optimized for extremely short lock durations (copying a small vector or a few floats).

## Audio Engine Internals

The `AudioEngine` (`imbolc-core/src/audio/engine/mod.rs`) acts as the "driver" for `scsynth`.

### Node Management & Routing Graph
Imbolc enforces a strict topological sort using SuperCollider Groups to ensure signal flow correctness within a single audio block:
1.  **Group 100 (Sources):** Oscillators, Samplers, Audio Input.
2.  **Group 200 (Processing):** Filters, Insert Effects, Mixer processing.
3.  **Group 300 (Output):** Master bus, Hardware output.
4.  **Group 400 (Record):** Disk recording (DiskOut).

This guarantees that a signal generated in Group 100 is available for processing in Group 200 immediately, preventing one-block latency delays between modules.

### Voice Allocation
Polyphony is managed via `VoiceChain`s. When a note plays:
1.  `AudioEngine` allocates a new `Group` inside the **Sources** group.
2.  It allocates unique audio/control buses for that voice.
3.  It creates a chain of synths (MIDI controls -> Oscillator/Sampler -> Envelope) within that voice group.
4.  It sends the entire setup as a single OSC bundle.

## Key Files Guide

*   `imbolc-core/src/audio/handle.rs`: The main thread's interface to the audio system.
*   `imbolc-core/src/audio/audio_thread.rs`: The dedicated thread loop.
*   `imbolc-core/src/audio/playback.rs`: The sequencer logic that calculates ticks and offsets.
*   `imbolc-core/src/audio/osc_client.rs`: UDP socket management and NTP timestamp calculation.
*   `imbolc-core/src/audio/engine/mod.rs`: The central `AudioEngine` struct managing the server state.
*   `imbolc-core/src/audio/engine/voices.rs`: Logic for spawning and releasing polyphonic voices.