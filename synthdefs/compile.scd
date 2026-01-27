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
SynthDef(\tuidaw_saw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Saw.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.asr(0.01, 1, 0.1), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_sin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = SinOsc.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.asr(0.01, 1, 0.1), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_sqr, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Pulse.ar(freqSig, 0.5) * amp * velSig;
    var env = EnvGen.kr(Env.asr(0.01, 1, 0.1), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\tuidaw_tri, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = LFTri.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.asr(0.01, 1, 0.1), gateSig);
    Out.ar(out, (sig * env) ! 2);
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
// LFO - Control rate oscillator
// ============================================================================
SynthDef(\tuidaw_lfo, { |out=0, rate=1, depth=0.5|
    var sig = SinOsc.kr(rate) * depth;
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

"Done! All SynthDefs written to: ".post;
dir.postln;
0.exit;
)
