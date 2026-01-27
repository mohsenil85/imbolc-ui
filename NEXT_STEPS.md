# Next Steps

Roadmap for tuidaw development.

## Target User Personas

Five user types we want to support, with their key needs and current status.

### 1. Composer
*"I want to write music with a piano roll and export the track."*

| Need | Status | Requires |
|------|--------|----------|
| Piano roll / step sequencer | Not implemented | SequencerPane, Track/Step state |
| Timeline transport (play/stop/loop) | Not implemented | Scheduler, clock sync |
| Audio export (WAV/MP3) | Not implemented | Render pipeline or SC NRT |

**Priority:** High - most "DAW-like" experience

---

### 2. Gigging Musician
*"I want to plug in a MIDI keyboard and play live shows."*

| Need | Status | Requires |
|------|--------|----------|
| MIDI device input | Not implemented | `midir` crate |
| Note → synth triggering | Not implemented | Velocity/gate handling |
| Low-latency playback | Supported | SuperCollider handles this |
| Preset switching | Partial | Save/load works, no preset UI |

**Priority:** Medium - moderate effort with `midir`

---

### 3. Recorder
*"I want to plug in a mic/guitar and record separate tracks."*

| Need | Status | Requires |
|------|--------|----------|
| Audio input capture | Not implemented | `cpal` crate |
| Multi-track recording | Not implemented | Track state, waveform display |
| Mixdown/bounce | Not implemented | Export pipeline |
| Overdub recording | Not implemented | Playback + record sync |

**Priority:** Lower - most complex, needs `cpal` + recording state

---

### 4. Sampler
*"I want to browse samples and assign them to pads/keys."*

| Need | Status | Requires |
|------|--------|----------|
| File browser pane | Not implemented | Directory navigation UI |
| Sample preview | Not implemented | SC buffer playback |
| Buffer management | Not implemented | `/b_allocRead` OSC commands |
| Pad/key assignment | Not implemented | Mapping UI, MIDI note triggers |
| Sampler module type | Not implemented | New ModuleType + SynthDef |

**Priority:** Medium - builds on existing SC integration

---

### 5. VST User
*"I want to use my existing VST plugins."*

| Need | Status | Requires |
|------|--------|----------|
| VST plugin loading | Not implemented | SC VSTPlugin extension |
| Plugin scanning | Not implemented | `VSTPlugin.search` |
| Parameter control | Not implemented | OSC to VSTPlugin |
| External editor launch | Not implemented | Platform-specific UI hosting |
| Preset management | Not implemented | VST state save/load |

**Priority:** Lower - requires SC extension installation, complex UI

---

### Persona Priority Matrix

| Persona | Effort | Impact | Recommended Phase |
|---------|--------|--------|-------------------|
| Composer | Medium | High | Phase 7 (Sequencer) |
| Gigging Musician | Low | Medium | Phase 8 (MIDI) |
| Sampler | Medium | Medium | Phase 9 |
| Recorder | High | Medium | Phase 10 |
| VST User | High | Low | Future |

---

## Current State

**Phase 6.5 complete:**
- UI engine with ratatui backend
- State types: `Module`, `ModuleType`, `Param`, `RackState`, `MixerState`
- Action/Effect enums for command pattern
- 7 panes: Rack, Add, Edit, Server, Mixer, Home, Help
- Pane communication fully wired
- SQLite persistence with normalized schema
- Module connections with signal routing
- **SuperCollider audio engine integration**
- **Mixer with 64 channels + 8 buses**

---

## What's Next? (Pick One)

### ~~Option 1: Audio Engine (Phase 6)~~ COMPLETE

### ~~Option 2: Mixer View~~ COMPLETE

### Option 3: Sequencer (Phase 7)

Step sequencer for creating patterns.

**What it involves:**
- Track/Step state types
- SequencerPane with grid UI
- Step editing (note, velocity, gate)
- Transport controls (play/stop/loop)
- Timeline with lookahead scheduling

**End result:** Create a 16-step pattern, play it back, hear a sequence.

---

### Option 4: MIDI Input (Phase 8)

Connect a MIDI keyboard and play.

**What it involves:**
- `midir` crate for MIDI input
- Device selection UI
- Note → frequency mapping
- Velocity → amplitude scaling
- Gate on/off for envelopes

**End result:** Plug in a keyboard, play notes, hear sound.

---

### Option 5: Visual Improvements

Better connection display and validation.

**What it involves:**
- Show connection "wires" between modules in rack view
- Color-code ports by type (Audio=blue, Control=green, Gate=yellow)
- Validation feedback when connecting incompatible ports
- Connection list with delete selection

**End result:** Clearer visual feedback for signal flow.

---

### Option 6: Undo/Redo

Command history for undoing changes.

**What it involves:**
- Command pattern with inverse operations
- Undo stack and redo stack
- `u` to undo, `Ctrl+R` to redo
- Works for: add/delete module, param changes, connections

**End result:** Make a mistake, press `u`, it's undone.

---

### Option 7: Sampler Module

Load and play audio samples.

**What it involves:**
- File browser pane for navigating sample directories
- Buffer management via OSC (`/b_allocRead`, `/b_free`)
- Sampler SynthDef with pitch, start/end points, envelope
- Sample pool state tracking loaded buffers
- Optional: pad grid UI for triggering

**End result:** Browse to a kick drum sample, load it, trigger it from a pad or MIDI note.

---

### Option 8: VST Plugin Support

Load VST2/VST3 plugins as modules.

**What it involves:**
- Install SuperCollider VSTPlugin extension
- Plugin scanning and enumeration UI
- VSTPlugin SynthDef wrapper
- Parameter list or external editor launch
- Preset save/load for plugin state

**End result:** Load Serum/Vital/etc as a module, control parameters, save presets.

**Note:** Requires users to install [VSTPlugin](https://git.iem.at/pd/vstplugin) quark in SuperCollider.

---

### Option 9: Audio Recording

Record audio input to tracks.

**What it involves:**
- `cpal` crate for audio input capture
- Audio device selection UI
- Recording state and waveform display
- Write to WAV files
- Overdub with playback sync

**End result:** Plug in a guitar, hit record, see waveform, mix with synths.

---

## Completed Phases

### Phase 1: UI Foundation
- Ratatui backend with Graphics trait abstraction
- Input handling with InputSource trait
- Basic main loop

### Phase 2: State & Views
- Module, ModuleType, Param, RackState types
- Action/Effect enums
- RackPane, AddPane, EditPane

### Phase 3: Pane Communication
- AddModule action: AddPane → RackState
- EditModule/UpdateModuleParams: EditPane ↔ RackState
- Pane downcasting with as_any_mut()

### Phase 4: Persistence
- SQLite database with normalized schema
- Tables: schema_version, session, modules, module_params
- Save with `w` key, load with `o` key
- Default path: `~/.config/tuidaw/rack.tuidaw`
- Round-trip test verifies params survive save/load

### Phase 5: Module Connections
- Port definitions (Audio, Control, Gate) for each module type
- Connection and PortRef types for signal routing
- Connect mode UI with port selection (`c` to connect, `x` to disconnect)
- Connections table in SQLite persistence
- Validation: only output→input, ports must exist

### Phase 6: Audio Engine
- OSC client for communicating with scsynth (using `rosc` crate)
- SynthDef loading from `.scsyndef` files
- Module → synth node mapping
- Real-time parameter control via OSC
- ServerPane for start/stop/connect/disconnect
- Background SynthDef compilation via sclang

### Phase 6.5: Mixer View
- MixerPane with horizontal channel strip layout
- 64 input channels + 8 submix buses + master
- Level faders, mute/solo per channel
- Output routing (channel → bus or master)
- Scrollable view with keyboard navigation
