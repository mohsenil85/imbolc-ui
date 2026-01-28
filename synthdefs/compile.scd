// Run in SuperCollider to generate .scsyndef files
// All modules use bus-based routing for audio/control signals
(
var dir = thisProcess.nowExecutingPath.dirname;

// ============================================================================
// MIDI - Note source that outputs freq/gate/vel on control buses
// ============================================================================
SynthDef(\tuidaw_midi, { |freq_out=0, gate_out=0, vel_out=0, note=60, vel=0.8, gate=1|
    var env = EnvGen.kr(Env.asr(0.001, 1, 0.01), gate, doneAction: 2);
    ReplaceOut.kr(freq_out, note.midicps);
    ReplaceOut.kr(gate_out, env);
    ReplaceOut.kr(vel_out, vel);
}).writeDefFile(dir);

// ============================================================================
// Oscillators - Read freq/gate/vel from control buses, output to audio bus
// Uses Select.kr to choose between parameter value or bus input (-1 = use param)
// ============================================================================
SynthDef(\tuidaw_saw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Saw.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_sin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = SinOsc.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_sqr, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Pulse.ar(freqSig, 0.5) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_tri, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = LFTri.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// ============================================================================
// Audio Input - Live audio from hardware input
// ============================================================================
SynthDef(\tuidaw_audio_in, { |out=1024, channel=0, gain=1, test_tone=0, test_freq=440, strip_id=0|
    var input = SoundIn.ar(channel) * gain;
    var test = SinOsc.ar(test_freq) * 0.3 * test_tone;
    var sig = input + test;
    // Send peak amplitude for waveform display (10 times per second)
    SendPeakRMS.kr(sig, 10, 3, "/audio_in_level", strip_id);
    Out.ar(out, sig ! 2);
}).writeDefFile(dir);

// ============================================================================
// Sampler - Plays from buffer with variable rate (for scratching)
// ============================================================================
SynthDef(\tuidaw_sampler, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
                             rate=1.0, rate_in=(-1), amp=0.8, loop=0,
                             freq_in=(-1), gate_in=(-1), vel_in=(-1),
                             attack=0.001, decay=0.1, sustain=1.0, release=0.05|
    var rateSig = Select.kr(rate_in >= 0, [rate, In.kr(rate_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    // If freq_in is provided, calculate rate from pitch (relative to middle C = 60)
    var freqSig = Select.kr(freq_in >= 0, [60.midicps, In.kr(freq_in)]);
    var pitchRate = freqSig / 60.midicps;
    var finalRate = rateSig * pitchRate;
    var numFrames = BufFrames.kr(bufnum);
    var startFrame = sliceStart * numFrames;
    var endFrame = sliceEnd * numFrames;
    var phasor = Phasor.ar(
        trig: 0,
        rate: BufRateScale.kr(bufnum) * finalRate,
        start: startFrame,
        end: endFrame,
        resetPos: startFrame
    );
    var sig = BufRd.ar(2, bufnum, phasor, loop: loop);
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig, doneAction: 2);
    Out.ar(out, sig * amp * env * velSig);
}).writeDefFile(dir);

// Sampler one-shot (for triggering slices without MIDI control)
SynthDef(\tuidaw_sampler_oneshot, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
                                      rate=1.0, amp=0.8, gate=1|
    var numFrames = BufFrames.kr(bufnum);
    var startFrame = sliceStart * numFrames;
    var endFrame = sliceEnd * numFrames;
    var phasor = Phasor.ar(
        trig: 0,
        rate: BufRateScale.kr(bufnum) * rate,
        start: startFrame,
        end: endFrame,
        resetPos: startFrame
    );
    var sig = BufRd.ar(2, bufnum, phasor, loop: 0);
    var env = EnvGen.kr(Env.asr(0.001, 1, 0.01), gate, doneAction: 2);
    // Free when phasor reaches end
    var done = (phasor >= (endFrame - 1));
    FreeSelf.kr(done);
    Out.ar(out, sig * amp * env);
}).writeDefFile(dir);

// ============================================================================
// Filters - Read from audio bus, optional cutoff modulation, write to audio bus
// ============================================================================
SynthDef(\tuidaw_lpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.1);
    Out.ar(out, RLPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\tuidaw_hpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.1);
    Out.ar(out, RHPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\tuidaw_bpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.01);
    Out.ar(out, BPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

// ============================================================================
// ADSR Envelope - Gate input, control output
// ============================================================================
SynthDef(\tuidaw_adsr, { |gate_in=(-1), out=0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, gate=0|
    var gateSig = Select.kr(gate_in >= 0, [gate, In.kr(gate_in)]);
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.kr(out, env);
}).writeDefFile(dir);

// ============================================================================
// LFO - Control rate oscillator with multiple shapes
// shape: 0=sine, 1=square, 2=saw, 3=triangle
// ============================================================================
SynthDef(\tuidaw_lfo, { |out=0, rate=1, depth=0.5, shape=0|
    var sine = SinOsc.kr(rate);
    var square = LFPulse.kr(rate).range(-1, 1);
    var saw = LFSaw.kr(rate);
    var tri = LFTri.kr(rate);
    var sig = Select.kr(shape, [sine, square, saw, tri]) * depth;
    Out.kr(out, sig);
}).writeDefFile(dir);

// ============================================================================
// Effects - Audio in/out with bus routing
// ============================================================================
SynthDef(\tuidaw_delay, { |in=1024, out=1026, time=0.3, feedback=0.5, mix=0.3|
    var sig = In.ar(in, 2);
    var delayed = CombL.ar(sig, 2.0, time, feedback * 4);
    Out.ar(out, (sig * (1 - mix)) + (delayed * mix));
}).writeDefFile(dir);

SynthDef(\tuidaw_reverb, { |in=1024, out=1026, room=0.5, damp=0.5, mix=0.3|
    var sig = In.ar(in, 2);
    var wet = FreeVerb2.ar(sig[0], sig[1], mix, room, damp);
    Out.ar(out, wet);
}).writeDefFile(dir);

// Gate/Tremolo effect
// shape: 0=sine (tremolo), 1=square (hard gate), 2=saw (ramp)
SynthDef(\tuidaw_gate, { |in=1024, out=1026, rate=4, depth=1, shape=1|
    var sig = In.ar(in, 2);
    var sine = SinOsc.kr(rate).range(1 - depth, 1);
    var square = LFPulse.kr(rate, width: 0.5).range(1 - depth, 1);
    var saw = LFSaw.kr(rate).range(1 - depth, 1);
    var lfo = Select.kr(shape, [sine, square, saw]);
    Out.ar(out, sig * lfo);
}).writeDefFile(dir);

// ============================================================================
// Output - Final stage, reads from audio bus, writes to hardware out
// Includes level, mute, and pan controls for mixer integration
// ============================================================================
SynthDef(\tuidaw_output, { |in=1024, level=0.8, mute=0, pan=0|
    var sig = In.ar(in, 2);
    var panned = Balance2.ar(sig[0], sig[1], pan);
    Out.ar(0, panned * level * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Send - Reads from source bus, writes to a bus's audio bus at send level
// ============================================================================
SynthDef(\tuidaw_send, { |in=1024, out=1026, level=0.0|
    var sig = In.ar(in, 2);
    Out.ar(out, sig * level);
}).writeDefFile(dir);

// ============================================================================
// Bus Output - Reads from bus audio bus, applies level/mute/pan, writes to hw
// ============================================================================
SynthDef(\tuidaw_bus_out, { |in=1024, level=0.8, mute=0, pan=0|
    var sig = In.ar(in, 2);
    var panned = Balance2.ar(sig[0], sig[1], pan);
    Out.ar(0, panned * level * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Meter - Reads hardware output, sends peak/RMS via /reply
// ============================================================================
SynthDef(\tuidaw_meter, {
    SendPeakRMS.kr(In.ar(0, 2), 10, 3, "/meter");
}).writeDefFile(dir);

"Done! All SynthDefs written to: ".post;
dir.postln;
0.exit;
)
