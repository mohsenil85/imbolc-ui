// Run in SuperCollider to generate .scsyndef files
// All modules use bus-based routing for audio/control signals
(
var dir = thisProcess.nowExecutingPath.dirname;

// ============================================================================
// MIDI - Note source that outputs freq/gate/vel on control buses
// ============================================================================
SynthDef(\imbolc_midi, { |freq_out=0, gate_out=0, vel_out=0, note=60, freq=440, vel=0.8, gate=1|
    var env = EnvGen.kr(Env.asr(0.001, 1, 0.01), gate, doneAction: 2);
    ReplaceOut.kr(freq_out, freq);
    ReplaceOut.kr(gate_out, env);
    ReplaceOut.kr(vel_out, vel);
}).writeDefFile(dir);

// ============================================================================
// Oscillators - Read freq/gate/vel from control buses, output to audio bus
// Uses Select.kr to choose between parameter value or bus input (-1 = use param)
// ============================================================================
SynthDef(\imbolc_saw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Saw.ar(finalFreq) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\imbolc_sin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = SinOsc.ar(finalFreq) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\imbolc_sqr, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), width_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var widthMod = Select.kr(width_mod_in >= 0, [0, In.kr(width_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Pulse.ar(finalFreq, (0.5 + widthMod).clip(0.01, 0.99)) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

SynthDef(\imbolc_tri, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = LFTri.ar(finalFreq) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Noise — White/Pink/Brown/Crackle/Dust selectable via color param
SynthDef(\imbolc_noise, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, lag=0.02, color=0, density=20, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Select.ar(color, [WhiteNoise.ar, PinkNoise.ar, BrownNoise.ar, Crackle.ar(1.5), Dust.ar(density)]);
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * finalAmp * velSig * env) ! 2);
}).writeDefFile(dir);

// Pulse — Variable pulse width oscillator (PWM)
SynthDef(\imbolc_pulse, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, width=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), width_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var widthMod = Select.kr(width_mod_in >= 0, [0, In.kr(width_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Pulse.ar(finalFreq, (width + widthMod).clip(0.01, 0.99)) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// SuperSaw — 7x detuned saw stack spread stereo
SynthDef(\imbolc_supersaw, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, detune=0.3, mix=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var center = Saw.ar(finalFreq);
    var detuneAmt = detune * 0.02; // max ~2% detune
    var detuned = Array.fill(6, { |i|
        var offset = ((i - 2.5) / 2.5) * detuneAmt;
        Saw.ar(finalFreq * (1 + offset));
    });
    var spread = Splay.ar(detuned);
    var sig = ((center * (1 - mix)) + (spread * mix)) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, sig * env);
}).writeDefFile(dir);

// Sync — Hard-sync oscillator
SynthDef(\imbolc_sync, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, sync_ratio=2.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = SyncSaw.ar(finalFreq, finalFreq * sync_ratio) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Ring — Ring modulator (carrier × modulator)
SynthDef(\imbolc_ring, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, mod_ratio=2.0, mod_depth=0.5, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var carrier = SinOsc.ar(finalFreq);
    var modulator = SinOsc.ar(finalFreq * mod_ratio);
    var sig = ((carrier * (1 - mod_depth)) + (carrier * modulator * mod_depth)) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// FBSin — Self-modulating feedback sine
SynthDef(\imbolc_fbsin, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, feedback=0.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = SinOscFB.ar(finalFreq, feedback) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// FM — 2-operator FM synthesis (DX7 style)
SynthDef(\imbolc_fm, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, ratio=2.0, index=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var modFreq = finalFreq * ratio;
    var modulator = SinOsc.ar(modFreq) * index * finalFreq;
    var sig = SinOsc.ar(finalFreq + modulator) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// PhaseMod — Phase distortion synthesis (Casio CZ style)
SynthDef(\imbolc_phasemod, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, ratio=2.0, index=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var modulator = SinOsc.ar(finalFreq * ratio) * index;
    var sig = SinOsc.ar(finalFreq, modulator) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Pluck — Karplus-Strong string synthesis
SynthDef(\imbolc_pluck, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, decay=2.0, coef=0.3, attack=0.01, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var maxDelay = 20.reciprocal; // 20 Hz lowest pitch
    var sig = Pluck.ar(WhiteNoise.ar, gateSig, maxDelay, finalFreq.reciprocal, decay, coef) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, 0.0, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Formant — Vocal formant synthesis
SynthDef(\imbolc_formant, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, formant=800, bw=200, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Formant.ar(finalFreq, formant.lag(lag), bw.lag(lag)) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Gendy — Xenakis dynamic stochastic synthesis
SynthDef(\imbolc_gendy, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, lag=0.02, ampdist=1, durdist=1, minfreq=100, maxfreq=1000, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var sig = Gendy1.ar(ampdist, durdist, 1.0, 1.0, minfreq, maxfreq) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Chaos — Chaotic attractor oscillators (Henon/Lorenz)
SynthDef(\imbolc_chaos, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), amp=0.5, lag=0.02, model=0, chaos_freq=8000, chaos_param=1.3, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    var henon = HenonN.ar(chaos_freq, chaos_param, 0.3);
    var lorenz = LorenzL.ar(chaos_freq, chaos_param.linlin(0, 2, 8, 14));
    var sig = Select.ar(model, [henon, lorenz]) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Additive — Harmonic series synthesis
SynthDef(\imbolc_additive, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, harmonics=8, rolloff=1.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    // Build 32 harmonics, gate amplitude on whether index < harmonics
    var sig = Mix.fill(32, { |i|
        var n = i + 1;
        var harmAmp = (n <= harmonics) * (n ** rolloff.neg);
        SinOsc.ar(finalFreq * n, 0, harmAmp);
    });
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    sig = sig * finalAmp * velSig;
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// Wavetable — Morphing wavetable oscillator using VOsc
SynthDef(\imbolc_wavetable, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1), freq=440, amp=0.5, lag=0.02, position=0.0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var freqSig = Select.kr(freq_in >= 0, [freq.lag(lag), In.kr(freq_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var pitchMod = Select.kr(pitch_mod_in >= 0, [0, In.kr(pitch_mod_in)]);
    var detuneMod = Select.kr(detune_mod_in >= 0, [0, In.kr(detune_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalFreq = freqSig * (2 ** pitchMod) * (1 + (detuneMod * 0.01));
    var finalAmp = amp.lag(lag) * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    // VOsc morphs between adjacent buffer pairs; position 0-1 maps to bufnum range
    // Expects wavetable buffers loaded at bufnums 100-107 (8 tables)
    var bufpos = position.linlin(0, 1, 100, 106.99);
    var sig = VOsc.ar(bufpos, finalFreq) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * env) ! 2);
}).writeDefFile(dir);

// ============================================================================
// Audio Input - Live audio from hardware input
// ============================================================================
SynthDef(\imbolc_audio_in, { |out=1024, channel=0, gain=1, test_tone=0, test_freq=440, instrument_id=0|
    var input = SoundIn.ar(channel) * gain;
    var test = SinOsc.ar(test_freq) * 0.3 * test_tone;
    var sig = input + test;
    // Send peak amplitude for waveform display (10 times per second)
    SendPeakRMS.kr(sig, 10, 3, "/audio_in_level", instrument_id);
    Out.ar(out, sig ! 2);
}).writeDefFile(dir);

// ============================================================================
// Bus Input - Read audio from an internal mixer bus
// ============================================================================
SynthDef(\imbolc_bus_in, { |out=1024, in=1024, gain=1, instrument_id=0|
    var sig = In.ar(in, 2) * gain;
    SendPeakRMS.kr(sig, 10, 3, "/audio_in_level", instrument_id);
    Out.ar(out, sig);
}).writeDefFile(dir);

// ============================================================================
// Sampler - Plays from buffer with variable rate (for scratching)
// ============================================================================
SynthDef(\imbolc_sampler, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
                             rate=1.0, rate_in=(-1), amp=0.8, loop=0,
                             freq_in=(-1), gate_in=(-1), vel_in=(-1),
                             attack=0.001, decay=0.1, sustain=1.0, release=0.05,
                             amp_mod_in=(-1), srate_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|
    var rateSig = Select.kr(rate_in >= 0, [rate, In.kr(rate_in)]);
    var gateSig = Select.kr(gate_in >= 0, [1, In.kr(gate_in)]);
    var velSig = Select.kr(vel_in >= 0, [1, In.kr(vel_in)]);
    var ampMod = Select.kr(amp_mod_in >= 0, [0, In.kr(amp_mod_in)]);
    var srateMod = Select.kr(srate_mod_in >= 0, [0, In.kr(srate_mod_in)]);
    var attackMod = Select.kr(attack_mod_in >= 0, [0, In.kr(attack_mod_in)]);
    var releaseMod = Select.kr(release_mod_in >= 0, [0, In.kr(release_mod_in)]);
    var finalAmp = amp * (1 + ampMod).max(0);
    var finalAttack = (attack * (1 + attackMod)).max(0.001);
    var finalRelease = (release * (1 + releaseMod)).max(0.001);
    // If freq_in is provided, calculate rate from pitch (relative to middle C = 60)
    var freqSig = Select.kr(freq_in >= 0, [60.midicps, In.kr(freq_in)]);
    var pitchRate = freqSig / 60.midicps;
    var finalRate = rateSig * pitchRate * (1 + srateMod);
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
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig, doneAction: 2);
    Out.ar(out, sig * finalAmp * env * velSig);
}).writeDefFile(dir);

// Sampler one-shot (for triggering slices without MIDI control)
SynthDef(\imbolc_sampler_oneshot, { |out=1024, bufnum=0, sliceStart=0, sliceEnd=1,
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
SynthDef(\imbolc_lpf, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var finalRes = (resonance.lag(lag) + resMod).clip(0, 1);
    var q = finalRes.linlin(0, 1, 1, 0.1);
    Out.ar(out, RLPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\imbolc_hpf, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var finalRes = (resonance.lag(lag) + resMod).clip(0, 1);
    var q = finalRes.linlin(0, 1, 1, 0.1);
    Out.ar(out, RHPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

SynthDef(\imbolc_bpf, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var finalRes = (resonance.lag(lag) + resMod).clip(0, 1);
    var q = finalRes.linlin(0, 1, 1, 0.01);
    Out.ar(out, BPF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

// ============================================================================
// ADSR Envelope - Gate input, control output
// ============================================================================
SynthDef(\imbolc_adsr, { |gate_in=(-1), out=0, attack=0.01, decay=0.1, sustain=0.7, release=0.3, gate=0|
    var gateSig = Select.kr(gate_in >= 0, [gate, In.kr(gate_in)]);
    var env = EnvGen.kr(Env.adsr(attack, decay, sustain, release), gateSig);
    Out.kr(out, env);
}).writeDefFile(dir);

// ============================================================================
// LFO - Control rate oscillator with multiple shapes
// shape: 0=sine, 1=square, 2=saw, 3=triangle
// ============================================================================
SynthDef(\imbolc_lfo, { |out=0, rate=1, depth=0.5, shape=0|
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
SynthDef(\imbolc_delay, { |in=1024, out=1026, time=0.3, feedback=0.5, mix=0.3, lag=0.02, time_mod_in=(-1), feedback_mod_in=(-1)|
    var sig = In.ar(in, 2);
    var timeMod = Select.kr(time_mod_in >= 0, [0, In.kr(time_mod_in)]);
    var fbMod = Select.kr(feedback_mod_in >= 0, [0, In.kr(feedback_mod_in)]);
    var finalTime = (time.lag(lag) * (1 + timeMod)).clip(0.001, 2.0);
    var finalFb = (feedback.lag(lag) + fbMod).clip(0, 1);
    var delayed = CombL.ar(sig, 2.0, finalTime, finalFb * 4);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (delayed * mix.lag(lag)));
}).writeDefFile(dir);

SynthDef(\imbolc_reverb, { |in=1024, out=1026, room=0.5, damp=0.5, mix=0.3, lag=0.02, mix_mod_in=(-1)|
    var sig = In.ar(in, 2);
    var mixMod = Select.kr(mix_mod_in >= 0, [0, In.kr(mix_mod_in)]);
    var finalMix = (mix.lag(lag) + mixMod).clip(0, 1);
    var wet = FreeVerb2.ar(sig[0], sig[1], finalMix, room.lag(lag), damp.lag(lag));
    Out.ar(out, wet);
}).writeDefFile(dir);

// Gate/Tremolo effect
// shape: 0=sine (tremolo), 1=square (hard gate), 2=saw (ramp)
SynthDef(\imbolc_gate, { |in=1024, out=1026, rate=4, depth=1, shape=1, lag=0.02, rate_mod_in=(-1)|
    var sig = In.ar(in, 2);
    var rateMod = Select.kr(rate_mod_in >= 0, [0, In.kr(rate_mod_in)]);
    var finalRate = (rate.lag(lag) * (1 + rateMod)).max(0.1);
    var depthLag = depth.lag(lag);
    var sine = SinOsc.kr(finalRate).range(1 - depthLag, 1);
    var square = LFPulse.kr(finalRate, width: 0.5).range(1 - depthLag, 1);
    var saw = LFSaw.kr(finalRate).range(1 - depthLag, 1);
    var lfo = Select.kr(shape, [sine, square, saw]);
    Out.ar(out, sig * lfo);
}).writeDefFile(dir);

// Tape Compressor - warm analog-style compression with saturation
SynthDef(\imbolc_tape_comp, { |in=1024, out=1026, drive=1.5, threshold=0.5, ratio=3, makeup=1.0, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var driven = (sig * drive.lag(lag)).tanh;
    var compressed = Compander.ar(driven, driven,
        thresh: threshold,
        slopeBelow: 1.0,
        slopeAbove: ratio.reciprocal,
        clampTime: 0.01,
        relaxTime: 0.1
    );
    var output = compressed * makeup.lag(lag);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (output * mix.lag(lag)));
}).writeDefFile(dir);

// Sidechain Compressor - driven by external bus signal
SynthDef(\imbolc_sc_comp, { |in=1024, out=1026, sidechain_in=0, threshold=0.3, ratio=4, attack=0.01, release=0.1, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var scSig = Select.ar(sidechain_in > 0, [sig, In.ar(sidechain_in, 2)]);
    var compressed = Compander.ar(sig, scSig,
        thresh: threshold,
        slopeBelow: 1.0,
        slopeAbove: ratio.reciprocal,
        clampTime: attack,
        relaxTime: release
    );
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (compressed * mix.lag(lag)));
}).writeDefFile(dir);

// Chorus — DelayL + SinOsc modulation
SynthDef(\imbolc_chorus, { |in=1024, out=1026, rate=0.5, depth=0.005, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var modL = SinOsc.kr(rate, 0) * depth;
    var modR = SinOsc.kr(rate, 0.5pi) * depth;
    var wet = [
        DelayL.ar(sig[0], 0.05, (depth + modL).clip(0.0001, 0.05)),
        DelayL.ar(sig[1], 0.05, (depth + modR).clip(0.0001, 0.05))
    ];
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Flanger — CombL with short modulated delay
SynthDef(\imbolc_flanger, { |in=1024, out=1026, rate=0.3, depth=0.003, feedback=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mod = SinOsc.kr(rate) * depth;
    var delayTime = (depth + mod).clip(0.0001, 0.01);
    var wet = CombL.ar(sig, 0.01, delayTime, feedback.lag(lag) * 0.5);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Phaser — AllpassL chain with LFO sweep
SynthDef(\imbolc_phaser, { |in=1024, out=1026, rate=0.5, depth=0.5, stages=4, feedback=0.3, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var lfo = SinOsc.kr(rate).range(200, 4000);
    var wet = sig;
    // 6 allpass stages, gated by stages param
    6.do { |i|
        var active = (i < stages);
        var freq = lfo * ((i + 1) / 6);
        var delay = freq.reciprocal.clip(0.00001, 0.01);
        wet = Select.ar(active, [wet, AllpassL.ar(wet, 0.01, delay, 0.001)]);
    };
    wet = wet + (LocalIn.ar(2) * feedback.lag(lag));
    LocalOut.ar(wet);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Tremolo — Amplitude modulation, shape-selectable
// shape: 0=sine, 1=square, 2=saw
SynthDef(\imbolc_tremolo, { |in=1024, out=1026, rate=4, depth=0.5, shape=0, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var sine = SinOsc.kr(rate).range(1 - depth, 1);
    var square = LFPulse.kr(rate, width: 0.5).range(1 - depth, 1);
    var saw = LFSaw.kr(rate).range(1 - depth, 1);
    var lfo = Select.kr(shape, [sine, square, saw]);
    var wet = sig * lfo;
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Distortion — hard clip / soft clip / foldback
// mode: 0=hard clip, 1=soft clip (tanh), 2=foldback
SynthDef(\imbolc_distortion, { |in=1024, out=1026, drive=2, mode=0, tone=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var driven = sig * drive.lag(lag);
    var hard = driven.clip2(1);
    var soft = driven.tanh;
    var fold = driven.fold2(1);
    var distorted = Select.ar(mode, [hard, soft, fold]);
    var toneFreq = tone.lag(lag).linexp(0, 1, 300, 12000);
    var filtered = LPF.ar(distorted, toneFreq);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (filtered * mix.lag(lag)));
}).writeDefFile(dir);

// Bitcrusher — Latch + bit reduction
SynthDef(\imbolc_bitcrusher, { |in=1024, out=1026, rate=8000, bits=8, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var crushed = Latch.ar(sig, Impulse.ar(rate));
    var bitDepth = (2 ** bits);
    crushed = (crushed * bitDepth).round / bitDepth;
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (crushed * mix.lag(lag)));
}).writeDefFile(dir);

// Wavefolder — fold2 with drive
SynthDef(\imbolc_wavefolder, { |in=1024, out=1026, drive=1, symmetry=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var offset = (symmetry.lag(lag) - 0.5) * 0.5;
    var driven = (sig + offset) * drive.lag(lag);
    var folded = driven.fold2(1);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (folded * mix.lag(lag)));
}).writeDefFile(dir);

// Saturator — tanh saturation + tone filter
SynthDef(\imbolc_saturator, { |in=1024, out=1026, drive=1.5, color=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var saturated = (sig * drive.lag(lag)).tanh;
    var toneFreq = color.lag(lag).linexp(0, 1, 500, 15000);
    var filtered = LPF.ar(saturated, toneFreq);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (filtered * mix.lag(lag)));
}).writeDefFile(dir);

// Tilt EQ — BLowShelf + BHiShelf complementary
SynthDef(\imbolc_tilt_eq, { |in=1024, out=1026, tilt=0, frequency=1000, lag=0.02|
    var sig = In.ar(in, 2);
    var tiltDb = tilt.lag(lag) * 12; // -12 to +12 dB
    var lo = BLowShelf.ar(sig, frequency.lag(lag), 1, tiltDb);
    var result = BHiShelf.ar(lo, frequency.lag(lag), 1, tiltDb.neg);
    Out.ar(out, result);
}).writeDefFile(dir);

// Stereo Widener — Mid/Side processing
SynthDef(\imbolc_stereo_widener, { |in=1024, out=1026, width=1, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var mid = (sig[0] + sig[1]) * 0.5;
    var side = (sig[0] - sig[1]) * 0.5;
    var widthLag = width.lag(lag);
    var wet = [mid + (side * widthLag), mid - (side * widthLag)];
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Frequency Shifter — FreqShift UGen
SynthDef(\imbolc_freq_shifter, { |in=1024, out=1026, shift_hz=0, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var wet = FreqShift.ar(sig, shift_hz.lag(lag));
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Limiter — Limiter UGen
SynthDef(\imbolc_limiter, { |in=1024, out=1026, threshold=0.9, release=0.01, ceiling=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var limited = Limiter.ar(sig, threshold.lag(lag), release.lag(lag));
    Out.ar(out, limited * ceiling.lag(lag));
}).writeDefFile(dir);

// Pitch Shifter — PitchShift UGen
SynthDef(\imbolc_pitch_shifter, { |in=1024, out=1026, shift=0, window=0.2, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var ratio = 2 ** (shift.lag(lag) / 12); // semitones to ratio
    var wet = PitchShift.ar(sig, window.lag(lag), ratio);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Vinyl — DelayL wow/flutter + Dust crackle + HPF hiss
SynthDef(\imbolc_vinyl, { |in=1024, out=1026, wow=0.3, flutter=0.3, noise=0.1, hiss=0.05, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    // Wow (slow pitch modulation) and flutter (fast pitch modulation)
    var wowMod = SinOsc.kr(0.3 + (wow * 0.5)) * wow * 0.003;
    var flutterMod = SinOsc.kr(6 + (flutter * 4)) * flutter * 0.001;
    var totalMod = wowMod + flutterMod;
    var modulated = DelayL.ar(sig, 0.05, (0.005 + totalMod).clip(0.0001, 0.05));
    // Crackle noise (Dust)
    var crackle = Dust.ar(3 + (noise * 20)) * noise * 0.15;
    // Hiss (filtered noise)
    var hissNoise = HPF.ar(WhiteNoise.ar(hiss * 0.05), 5000);
    var wet = modulated + crackle + hissNoise;
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Cabinet — Shaped LPF profiles + mild saturation
// type: 0=small combo, 1=large combo, 2=4x12 stack, 3=open back
SynthDef(\imbolc_cabinet, { |in=1024, out=1026, type=0, tone=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var toneFreq = tone.lag(lag);
    // Different cabinet profiles with different LPF cutoffs and resonance
    var small = LPF.ar(sig.tanh, toneFreq.linexp(0, 1, 800, 5000));
    var large = LPF.ar(sig.tanh, toneFreq.linexp(0, 1, 600, 4000));
    var stack = LPF.ar(BPeakEQ.ar(sig.tanh, 2000, 1, 3), toneFreq.linexp(0, 1, 500, 3500));
    var open = LPF.ar(sig.tanh, toneFreq.linexp(0, 1, 1000, 6000));
    var wet = Select.ar(type, [small, large, stack, open]);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Granular Delay — BufWr + TGrains into LocalBuf
SynthDef(\imbolc_granular_delay, { |in=1024, out=1026, time=0.3, grain_size=0.1, density=10, pitch=0, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var bufFrames = 44100 * 2; // 2 second buffer
    var buf = LocalBuf(bufFrames, 1);
    var phase = Phasor.ar(0, 1, 0, bufFrames);
    BufWr.ar(mono, buf, phase);
    var readPos = (phase - (time.lag(lag) * SampleRate.ir)).wrap(0, bufFrames);
    var pitchRatio = 2 ** (pitch.lag(lag) / 12);
    var trig = Impulse.ar(density);
    var wet = TGrains.ar(2, trig, buf, pitchRatio, readPos / SampleRate.ir, grain_size.lag(lag), 0, 0.5);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Granular Freeze — Frozen grain position + scatter
SynthDef(\imbolc_granular_freeze, { |in=1024, out=1026, grain_size=0.1, density=10, pitch=0, spread=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var bufFrames = 44100 * 2;
    var buf = LocalBuf(bufFrames, 1);
    var phase = Phasor.ar(0, 1, 0, bufFrames);
    BufWr.ar(mono, buf, phase);
    var pitchRatio = 2 ** (pitch.lag(lag) / 12);
    var trig = Impulse.ar(density);
    // Scatter grain position around current write position
    var scatter = TRand.ar(0, spread.lag(lag) * bufFrames, trig);
    var grainPos = (phase - scatter).wrap(0, bufFrames) / SampleRate.ir;
    var wet = TGrains.ar(2, trig, buf, pitchRatio, grainPos, grain_size.lag(lag), 0, 0.5);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// ============================================================================
// Convolution Reverb - Uses loaded impulse response buffer
// ir_buffer: SC buffer number of loaded IR (-1 = bypass/passthrough)
// ============================================================================
SynthDef(\imbolc_convolution_reverb, { |in=1024, out=1026, ir_buffer=(-1), mix=0.3, predelay=0.0, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var delayed = DelayN.ar(mono, 0.5, predelay.lag(lag));
    // Convolution2 uses partitioned convolution (FFT size 2048)
    var conv = Convolution2.ar(delayed, ir_buffer, 0, 2048);
    var wet = [conv, conv]; // mono IR -> stereo
    // If ir_buffer < 0, pass through dry signal
    var hasIR = (ir_buffer >= 0);
    var output = Select.ar(hasIR, [sig, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag))]);
    Out.ar(out, output);
}).writeDefFile(dir);

// ============================================================================
// Output - Final stage, reads from audio bus, writes to hardware out
// Includes level, mute, and pan controls for mixer integration
// ============================================================================
SynthDef(\imbolc_output, { |in=1024, level=0.8, mute=0, pan=0, lag=0.02, pan_mod_in=(-1)|
    var sig = In.ar(in, 2);
    var panMod = Select.kr(pan_mod_in >= 0, [0, In.kr(pan_mod_in)]);
    var finalPan = (pan.lag(lag) + panMod).clip(-1, 1);
    var panned = Balance2.ar(sig[0], sig[1], finalPan);
    Out.ar(0, panned * level.lag(lag) * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Send - Reads from source bus, writes to a bus's audio bus at send level
// ============================================================================
SynthDef(\imbolc_send, { |in=1024, out=1026, level=0.0, lag=0.02, level_mod_in=(-1)|
    var sig = In.ar(in, 2);
    var levelMod = Select.kr(level_mod_in >= 0, [0, In.kr(level_mod_in)]);
    var finalLevel = (level.lag(lag) + levelMod).clip(0, 1);
    Out.ar(out, sig * finalLevel);
}).writeDefFile(dir);

// ============================================================================
// Bus Output - Reads from bus audio bus, applies level/mute/pan, writes to hw
// ============================================================================
SynthDef(\imbolc_bus_out, { |in=1024, level=0.8, mute=0, pan=0, lag=0.02|
    var sig = In.ar(in, 2);
    var panned = Balance2.ar(sig[0], sig[1], pan.lag(lag));
    Out.ar(0, panned * level.lag(lag) * (1 - mute));
}).writeDefFile(dir);

// ============================================================================
// Meter - Reads hardware output, sends peak/RMS via /reply
// ============================================================================
SynthDef(\imbolc_meter, {
    SendPeakRMS.kr(In.ar(0, 2), 10, 3, "/meter");
}).writeDefFile(dir);

// ============================================================================
// Spectrum Analyzer - 7-band frequency analysis via bandpass filters
// Sends amplitude data at ~20Hz via SendReply
// ============================================================================
SynthDef(\imbolc_spectrum, {
    var sig = In.ar(0, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var freqs = #[60, 150, 400, 1000, 2500, 6000, 15000];
    var bws = #[0.8, 0.7, 0.6, 0.5, 0.5, 0.5, 0.5]; // wider bandwidth at low freqs
    var amps = Array.fill(7, { |i|
        Amplitude.kr(BPF.ar(mono, freqs[i], bws[i]), 0.05, 0.2);
    });
    SendReply.kr(Impulse.kr(20), '/spectrum', amps);
}).writeDefFile(dir);

// ============================================================================
// LUFS Meter - K-weighted loudness measurement
// Sends peak and RMS at ~20Hz via SendPeakRMS
// ============================================================================
SynthDef(\imbolc_lufs_meter, {
    var sig = In.ar(0, 2);
    // Approximate K-weighting: high-shelf boost + high-pass
    var weighted = HPF.ar(BHiShelf.ar(sig, 1500, 1, 4), 38);
    SendPeakRMS.kr(weighted, 20, 3, "/lufs");
}).writeDefFile(dir);

// ============================================================================
// Scope - Sends peak amplitude at ~30Hz for oscilloscope display
// ============================================================================
SynthDef(\imbolc_scope, {
    var sig = In.ar(0, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    // Bipolar peak: track both positive and negative peaks
    var peak = Amplitude.kr(mono, 0.001, 0.03);
    // Capture sign by checking current sample polarity
    var sign = Latch.kr(mono, Impulse.kr(30));
    SendReply.kr(Impulse.kr(30), '/scope', [sign.sign * peak]);
}).writeDefFile(dir);

// ============================================================================
// Disk Recorder - Writes stereo audio from a bus to a disk-backed buffer
// ============================================================================
SynthDef(\imbolc_disk_record, { |bufnum=0, in=0|
    DiskOut.ar(bufnum, In.ar(in, 2));
}).writeDefFile(dir);

"Done! All SynthDefs written to: ".post;
dir.postln;
0.exit;
)
