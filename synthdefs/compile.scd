// Run in SuperCollider to generate .scsyndef files
// All modules use bus-based routing for audio/control signals
(
var dir = thisProcess.nowExecutingPath.dirname;

// ============================================================================
// MIDI - Note source that outputs freq/gate/vel on control buses
// ============================================================================
SynthDef(\ilex_midi, { |freq_out=0, gate_out=0, vel_out=0, note=60, freq=440, vel=0.8, gate=1|
    var env = EnvGen.kr(Env.asr(0.001, 1, 0.01), gate, doneAction: 2);
    ReplaceOut.kr(freq_out, freq);
    ReplaceOut.kr(gate_out, env);
    ReplaceOut.kr(vel_out, vel);
}).writeDefFile(dir);

// ============================================================================
// Oscillators - Read freq/gate/vel from control buses, output to audio bus
// Uses Select.kr to choose between parameter value or bus input (-1 = use param)
// ============================================================================
SynthDef(\ilex_saw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Saw.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\ilex_sin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = SinOsc.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\ilex_sqr, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Pulse.ar(freqSig, 0.5) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\ilex_tri, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = LFTri.ar(freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Noise — White/Pink/Brown/Crackle/Dust selectable via color param
SynthDef(\ilex_noise, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, color=0, density=20, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Select.ar(color, [WhiteNoise.ar, PinkNoise.ar, BrownNoise.ar, Crackle.ar(1.5), Dust.ar(density)]);
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * amp * velSig * env) ! 2);
}).writeDefFile(dir);

// Pulse — Variable pulse width oscillator (PWM)
SynthDef(\ilex_pulse, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, width=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Pulse.ar(freqSig, width) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// SuperSaw — 7x detuned saw stack spread stereo
SynthDef(\ilex_supersaw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, detune=0.3, mix=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var center = Saw.ar(freqSig);
    var detuneAmt = detune * 0.02; // max ~2% detune
    var detuned = Array.fill(6, { |i|
        var offset = ((i - 2.5) / 2.5) * detuneAmt;
        Saw.ar(freqSig * (1 + offset));
    });
    var spread = Splay.ar(detuned);
    var sig = ((center * (1 - mix)) + (spread * mix)) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, sig * env);
}).writeDefFile(dir);

// Sync — Hard-sync oscillator
SynthDef(\ilex_sync, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, sync_ratio=2.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = SyncSaw.ar(freqSig, freqSig * sync_ratio) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Ring — Ring modulator (carrier × modulator)
SynthDef(\ilex_ring, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, mod_ratio=2.0, mod_depth=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var carrier = SinOsc.ar(freqSig);
    var modulator = SinOsc.ar(freqSig * mod_ratio);
    var sig = ((carrier * (1 - mod_depth)) + (carrier * modulator * mod_depth)) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// FBSin — Self-modulating feedback sine
SynthDef(\ilex_fbsin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, feedback=0.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = SinOscFB.ar(freqSig, feedback) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// FM — 2-operator FM synthesis (DX7 style)
SynthDef(\ilex_fm, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, ratio=2.0, index=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var modFreq = freqSig * ratio;
    var modulator = SinOsc.ar(modFreq) * index * freqSig;
    var sig = SinOsc.ar(freqSig + modulator) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// PhaseMod — Phase distortion synthesis (Casio CZ style)
SynthDef(\ilex_phasemod, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, ratio=2.0, index=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var modulator = SinOsc.ar(freqSig * ratio) * index;
    var sig = SinOsc.ar(freqSig, modulator) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Pluck — Karplus-Strong string synthesis
SynthDef(\ilex_pluck, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, decay=2.0, coef=0.3, attack=0.01, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var maxDelay = 20.reciprocal; // 20 Hz lowest pitch
    var sig = Pluck.ar(WhiteNoise.ar, gateSig, maxDelay, freqSig.reciprocal, decay, coef) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, 0.0, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Formant — Vocal formant synthesis
SynthDef(\ilex_formant, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, formant=800, bw=200, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Formant.ar(freqSig, formant, bw) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Gendy — Xenakis dynamic stochastic synthesis
SynthDef(\ilex_gendy, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, ampdist=1, durdist=1, minfreq=100, maxfreq=1000, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var sig = Gendy1.ar(ampdist, durdist, 1.0, 1.0, minfreq, maxfreq) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Chaos — Chaotic attractor oscillators (Henon/Lorenz)
SynthDef(\ilex_chaos, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, model=0, chaos_freq=8000, chaos_param=1.3, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var henon = HenonN.ar(chaos_freq, chaos_param, 0.3);
    var lorenz = LorenzL.ar(chaos_freq, chaos_param.linlin(0, 2, 8, 14));
    var sig = Select.ar(model, [henon, lorenz]) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Additive — Harmonic series synthesis
SynthDef(\ilex_additive, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, harmonics=8, rolloff=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    // Build 32 harmonics, gate amplitude on whether index < harmonics
    var sig = Mix.fill(32, { |i|
        var n = i + 1;
        var harmAmp = (n <= harmonics) * (n ** rolloff.neg);
        SinOsc.ar(freqSig * n, 0, harmAmp);
    });
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    sig = sig * amp * velSig;
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Wavetable — Morphing wavetable oscillator using VOsc
SynthDef(\ilex_wavetable, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, position=0.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3|
    var freqSig = Select.kr(freq_in >= 0, [freq, In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    // VOsc morphs between adjacent buffer pairs; position 0-1 maps to bufnum range
    // Expects wavetable buffers loaded at bufnums 100-107 (8 tables)
    var bufpos = position.linlin(0, 1, 100, 106.99);
    var sig = VOsc.ar(bufpos, freqSig) * amp * velSig;
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// ============================================================================
// Audio Input - Live audio from hardware input
// ============================================================================
SynthDef(\ilex_audio_in, { |out=1024, channel=0, gain=1, test_tone=0, test_freq=440, strip_id=0|
    var input = SoundIn.ar(channel) * gain;
    var test = SinOsc.ar(test_freq) * 0.3 * test_tone;
    var sig = input + test;
    // Send peak amplitude for waveform display (10 times per second)
    SendPeakRMS.kr(sig, 10, 3, "/audio_in_level", strip_id);
    Out.ar(out, sig ! 2);
}).writeDefFile(dir);

// ============================================================================
// Bus Input - Read audio from an internal mixer bus
// ============================================================================
SynthDef(\ilex_bus_in, { |out=1024, in=1024, gain=1, strip_id=0|
    var sig = In.ar(in, 2) * gain;
    SendPeakRMS.kr(sig, 10, 3, "/audio_in_level", strip_id);
    Out.ar(out, sig);
}).writeDefFile(dir);

// ============================================================================
// Sampler - Plays from buffer with variable rate (for scratching)
// ============================================================================
SynthDef(\ilex_sampler, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
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
SynthDef(\ilex_sampler_oneshot, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
                                      rate=1.0, amp=0.8, gate=1|
    var numFrames = BufFrames.kr(bufnum);
    var startFrame = sliceStart * numFrames;
    var endFrame = sliceEnd * numFrames;
    
    // Calculate duration to drive Line and Envelope
    var bufRate = BufSampleRate.kr(bufnum) * rate.abs;
    var duration = (endFrame - startFrame).abs / bufRate;
    
    // Line.ar drives the pointer from start to end over duration
    var phasor = Line.ar(startFrame, endFrame, duration, doneAction: 2);
    
    var sig = BufRd.ar(2, bufnum, phasor, loop: 0);
    
    // Simple envelope to de-click
    var attack = 0.002;
    var release = 0.01;
    // Ensure sustain is not negative if duration is very short
    var sustain = (duration - attack - release).max(0);
    var env = EnvGen.kr(Env.linen(attack, sustain, release), doneAction: 2);
    
    Out.ar(out, sig * amp * env);
}).writeDefFile(dir);

// ============================================================================
// Filters - Read from audio bus, optional cutoff modulation, write to audio bus
// ============================================================================
SynthDef(\ilex_lpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.1);
    Out.ar(out, RLPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\ilex_hpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.1);
    Out.ar(out, RHPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\ilex_bpf, { |in=1024, out=1026, cutoff_mod_in=(-1), cutoff=1000, resonance=0.5|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var finalCutoff = (cutoff * (1 + cutoffMod)).clip(20, 20000);
    var q = resonance.linlin(0, 1, 1, 0.01);
    Out.ar(out, BPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

// ============================================================================
// ADSR Envelope - Gate input, control output
// ============================================================================
SynthDef(\ilex_adsr, { |gate_in=(-1), out=0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, gate=0|
    var gateSig = Select.kr(gate_in >= 0, [gate, In.kr(gate_in)]);
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.kr(out, env);
}).writeDefFile(dir);

// ============================================================================
// LFO - Control rate oscillator with multiple shapes
// shape: 0=sine, 1=square, 2=saw, 3=triangle
// ============================================================================
SynthDef(\ilex_lfo, { |out=0, rate=1, depth=0.5, shape=0|
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
SynthDef(\ilex_delay, { |in=1024, out=1026, time=0.3, feedback=0.5, mix=0.3|
    var sig = In.ar(in, 2);
    var delayed = CombL.ar(sig, 2.0, time, feedback * 4);
    Out.ar(out, (sig * (1 - mix)) + (delayed * mix));
}).writeDefFile(dir);

SynthDef(\ilex_reverb, { |in=1024, out=1026, room=0.5, damp=0.5, mix=0.3|
    var sig = In.ar(in, 2);
    var wet = FreeVerb2.ar(sig[0], sig[1], mix, room, damp);
    Out.ar(out, wet);
}).writeDefFile(dir);

// Gate/Tremolo effect
// shape: 0=sine (tremolo), 1=square (hard gate), 2=saw (ramp)
SynthDef(\ilex_gate, { |in=1024, out=1026, rate=4, depth=1, shape=1|
    var sig = In.ar(in, 2);
    var sine = SinOsc.kr(rate).range(1 - depth, 1);
    var square = LFPulse.kr(rate, width: 0.5).range(1 - depth, 1);
    var saw = LFSaw.kr(rate).range(1 - depth, 1);
    var lfo = Select.kr(shape, [sine, square, saw]);
    Out.ar(out, sig * lfo);
}).writeDefFile(dir);

// Tape Compressor - warm analog-style compression with saturation
SynthDef(\ilex_tape_comp, { |in=1024, out=1026, drive=1.5, threshold=0.5, ratio=3, makeup=1.0, mix=1.0|
    var sig = In.ar(in, 2);
    var driven = (sig * drive).tanh;
    var compressed = Compander.ar(driven, driven,
        thresh: threshold,
        slopeBelow: 1.0,
        slopeAbove: ratio.reciprocal,
        clampTime: 0.01,
        relaxTime: 0.1
    );
    var output = compressed * makeup;
    Out.ar(out, (sig * (1 - mix)) + (output * mix));
}).writeDefFile(dir);

// Sidechain Compressor - driven by external bus signal
SynthDef(\ilex_sc_comp, { |in=1024, out=1026, sidechain_in=0, threshold=0.3, ratio=4, attack=0.01, release=0.1, mix=1.0|
    var sig = In.ar(in, 2);
    var scSig = Select.ar(sidechain_in > 0, [sig, In.ar(sidechain_in, 2)]);
    var compressed = Compander.ar(sig, scSig,
        thresh: threshold,
        slopeBelow: 1.0,
        slopeAbove: ratio.reciprocal,
        clampTime: attack,
        relaxTime: release
    );
    Out.ar(out, (sig * (1 - mix)) + (compressed * mix));
}).writeDefFile(dir);

// ============================================================================
// Output - Final stage, reads from audio bus, writes to hardware out
// Includes level, mute, and pan controls for mixer integration
// ============================================================================
SynthDef(\ilex_output, { |in=1024, level=0.8, mute=0, pan=0|
    var sig = In.ar(in, 2);
    var panned = Balance2.ar(sig[0], sig[1], pan);
    Out.ar(0, panned * level * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Send - Reads from source bus, writes to a bus's audio bus at send level
// ============================================================================
SynthDef(\ilex_send, { |in=1024, out=1026, level=0.0|
    var sig = In.ar(in, 2);
    Out.ar(out, sig * level);
}).writeDefFile(dir);

// ============================================================================
// Bus Output - Reads from bus audio bus, applies level/mute/pan, writes to hw
// ============================================================================
SynthDef(\ilex_bus_out, { |in=1024, level=0.8, mute=0, pan=0|
    var sig = In.ar(in, 2);
    var panned = Balance2.ar(sig[0], sig[1], pan);
    Out.ar(0, panned * level * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Meter - Reads hardware output, sends peak/RMS via /reply
// ============================================================================
SynthDef(\ilex_meter, {
    SendPeakRMS.kr(In.ar(0, 2), 10, 3, "/meter");
}).writeDefFile(dir);

// ============================================================================
// Disk Recorder - Writes stereo audio from a bus to a disk-backed buffer
// ============================================================================
SynthDef(\ilex_disk_record, { |bufnum=0, in=0|
    DiskOut.ar(bufnum, In.ar(in, 2));
}).writeDefFile(dir);

"Done! All SynthDefs written to: ".post;
dir.postln;
0.exit;
)
