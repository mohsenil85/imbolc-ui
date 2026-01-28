# Next Steps

Roadmap for tuidaw development.

## Current State (January 2025)

The core DAW is functional with:

**Audio Engine:**
- SuperCollider integration via OSC
- Bus-based routing (audio + control buses)
- Strip architecture: source → filter → effects → output
- Polyphonic voice allocation per strip
- Real-time parameter control

**Instrument Strips:**
- Source types: Saw, Sin, Sqr, Tri, AudioIn, Sampler
- Filter types: LPF, HPF, BPF with cutoff/resonance
- Effects: Delay, Reverb, Gate (tremolo/hard gate)
- ADSR envelope per strip
- LFO with 15 target types (FilterCutoff wired, others defined)
- Level control per strip

**Piano Roll:**
- Multi-track note editing
- Playback with transport controls
- Note velocity and duration
- Per-strip tracks

**Persistence:**
- SQLite-based `.tuidaw` files
- Strips, notes, LFO settings, connections all saved

**UI:**
- Strip list pane with piano keyboard mode
- Strip editor with section-based navigation
- Piano roll pane
- Server status pane

---

## What's Next?

### High Priority

#### 1. Wire Remaining LFO Targets
*See `docs/lfo-targets-implementation.md` for full plan*

14 targets defined but not yet wired:
- **Easy:** FilterResonance, Pan, DelayFeedback, ReverbMix, SendLevel
- **Medium:** Amplitude, GateRate, SampleRate (scratching!)
- **Complex:** Pitch, Detune, DelayTime, Attack, Release, PulseWidth

**End result:** Full modulation capabilities for all strip parameters.

---

#### 2. MIDI Input
*Connect hardware MIDI controllers*

| Need | Status | Requires |
|------|--------|----------|
| MIDI device input | Not implemented | `midir` crate |
| Note triggering | Voice allocation exists | Hook to spawn_voice |
| CC mapping | Not implemented | Map CC to strip params |
| Pitch bend | Not implemented | Map to sampler rate (scratching) |

**End result:** Plug in a keyboard, play notes, use pitch bend for scratching.

---

#### 3. Sample Browser
*Load samples into sampler strips*

| Need | Status | Requires |
|------|--------|----------|
| File browser pane | Not implemented | Directory navigation UI |
| Sample preview | Not implemented | SC buffer playback |
| Slice editing | Not implemented | Waveform display, markers |

**End result:** Browse samples, load into sampler strip, create slices.

---

### Medium Priority

#### 4. Automation Recording
*Record parameter changes over time*

| Need | Status | Requires |
|------|--------|----------|
| Automation lanes | Not implemented | Data structures exist in plan |
| Recording mode | Not implemented | Capture param changes during playback |
| Playback interpolation | Not implemented | Apply values during tick loop |

**End result:** Record filter sweeps, mixer moves, LFO depth changes.

---

#### 5. Audio Export
*Render to WAV*

| Need | Status | Requires |
|------|--------|----------|
| Offline render | Not implemented | SC NRT mode or real-time capture |
| Progress UI | Not implemented | Render status display |

**End result:** Export your track as a WAV file.

---

#### 6. Undo/Redo
*Command history*

| Need | Status | Requires |
|------|--------|----------|
| Undo stack | Not implemented | Command pattern with inverses |
| Redo stack | Not implemented | Same |

**End result:** Press `u` to undo, `Ctrl+r` to redo.

---

### Lower Priority

#### 7. VST Plugin Support
*Load VST2/VST3 plugins*

Requires SuperCollider VSTPlugin extension. See `docs/vst-integration.md`.

#### 8. Multi-track Audio Recording
*Record live audio input to tracks*

Would need `cpal` crate for audio capture, waveform display, overdub sync.

---

## Completed Features

### Phase 1-6: Foundation
- Ratatui UI with Graphics abstraction
- State management (strips, connections, piano roll)
- SQLite persistence
- SuperCollider OSC integration
- Bus-based audio routing

### Phase 7: Strip Architecture
- Instrument strips replacing modular rack
- Source → Filter → Effects → Output chain
- ADSR envelopes
- Level/pan per strip

### Phase 8: Piano Roll
- Multi-track note editing
- Playback engine with transport
- Voice allocation (polyphony)

### Phase 9: LFO & Effects
- LFO module with shape selection
- 15 modulation targets defined
- Gate effect (tremolo/hard gate)
- FilterCutoff modulation wired

### Phase 10: Audio Input
- AudioIn source type
- Hardware input monitoring
- Test tone for debugging
