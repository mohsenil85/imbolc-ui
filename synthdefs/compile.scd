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

// Granular — Pitched granular synthesis using GrainSin (no buffer needed)
SynthDef(\imbolc_granular, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1),
    freq=440, amp=0.5, lag=0.02, grain_size=0.05, density=20, spread=0, pitch_rnd=0,
    attack=0.01, decay=0.1, sustain=0.7, release=0.3,
    amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|

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

    var trig = Dust.kr(density);
    var grainFreq = finalFreq * (2 ** (TRand.kr(-1, 1, trig) * pitch_rnd));
    var pan = TRand.kr(spread.neg, spread, trig);
    var sig = GrainSin.ar(2, trig, grain_size.lag(lag), grainFreq, pan) * finalAmp * velSig;
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, sig * env);
}).writeDefFile(dir);

// ============================================================================
// Physical Models - Bowed, Blown, Membrane
// ============================================================================

// Bowed — delay-line resonator excited by filtered noise
SynthDef(\imbolc_bowed, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1),
    freq=440, amp=0.5, lag=0.02, pressure=0.5, bow_pos=0.12,
    attack=0.01, decay=0.1, sustain=0.7, release=0.3,
    amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|

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

    var exciter = BrownNoise.ar * pressure.lag(lag) * gateSig;
    var sig = CombL.ar(exciter, 0.05, finalFreq.reciprocal * (1 - bow_pos.lag(lag)),
        pressure.lag(lag).linlin(0, 1, 0.5, 4.0));
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    sig = LPF.ar(sig, finalFreq * 4);
    Out.ar(out, (sig * finalAmp * velSig * env) ! 2);
}).writeDefFile(dir);

// Blown — jet noise excitation + waveguide delay
SynthDef(\imbolc_blown, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1),
    freq=440, amp=0.5, lag=0.02, pressure=0.5, embouchure=0.5,
    attack=0.01, decay=0.1, sustain=0.7, release=0.3,
    amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|

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

    var jet = WhiteNoise.ar * pressure.lag(lag) * gateSig;
    var embLag = embouchure.lag(lag);
    var jetClip = (jet * embLag.linlin(0, 1, 1, 4)).clip2(1);
    var sig = CombL.ar(jetClip, 0.05, finalFreq.reciprocal,
        pressure.lag(lag).linlin(0, 1, 0.2, 2.0));
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    sig = LPF.ar(sig, finalFreq * (2 + (embLag * 3)));
    Out.ar(out, (sig * finalAmp * velSig * env) ! 2);
}).writeDefFile(dir);

// Membrane — impulse into parallel modal resonators
SynthDef(\imbolc_membrane, { |out=1024, freq_in=(-1), gate_in=(-1), vel_in=(-1),
    freq=440, amp=0.5, lag=0.02, tension=0.05, loss=0.99,
    attack=0.001, decay=0.3, sustain=0.0, release=0.1,
    amp_mod_in=(-1), pitch_mod_in=(-1), detune_mod_in=(-1), attack_mod_in=(-1), release_mod_in=(-1)|

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

    var exciter = WhiteNoise.ar(0.5) * EnvGen.ar(Env.perc(0.001, tension.lag(lag)), gateSig);
    var mode1 = CombL.ar(exciter, 0.05, finalFreq.reciprocal, loss.lag(lag).linlin(0.9, 1.0, 0.1, 3.0));
    var mode2 = CombL.ar(exciter, 0.05, (finalFreq * 1.59).reciprocal, loss.lag(lag).linlin(0.9, 1.0, 0.05, 2.0));
    var mode3 = CombL.ar(exciter, 0.05, (finalFreq * 2.14).reciprocal, loss.lag(lag).linlin(0.9, 1.0, 0.02, 1.5));
    var sig = mode1 + (mode2 * 0.5) + (mode3 * 0.25);
    var env = EnvGen.kr(Env.adsr(finalAttack, decay, sustain, finalRelease), gateSig);
    Out.ar(out, (sig * finalAmp * velSig * env) ! 2);
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
    var isReverse = finalRate < 0;
    var phasorStart = Select.kr(isReverse, [startFrame, endFrame]);
    var phasorEnd = Select.kr(isReverse, [endFrame, startFrame]);
    var phasorReset = Select.kr(isReverse, [startFrame, endFrame]);
    var phasor = Phasor.ar(
        trig: 0,
        rate: BufRateScale.kr(bufnum) * finalRate,
        start: phasorStart,
        end: phasorEnd,
        resetPos: phasorReset
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
    var isReverse = rate < 0;
    var lineStart = Select.kr(isReverse, [startFrame, endFrame]);
    var lineEnd = Select.kr(isReverse, [endFrame, startFrame]);
    var phasor = Line.ar(lineStart, lineEnd, duration, doneAction: 2);
    
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

// Notch (Band-Reject)
SynthDef(\imbolc_notch, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var q = (resonance.lag(lag) + resMod).clip(0, 1).linlin(0, 1, 1, 0.01);
    Out.ar(out, BRF.ar(sig, finalCutoff, q));
}).writeDefFile(dir);

// Comb — cutoff maps to comb frequency, resonance to decay
SynthDef(\imbolc_comb, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=440, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var finalRes = (resonance.lag(lag) + resMod).clip(0, 1);
    var delay = finalCutoff.reciprocal.clip(0.00005, 0.05);
    var decayTime = finalRes.linlin(0, 1, 0.01, 2.0);
    Out.ar(out, CombC.ar(sig, 0.05, delay, decayTime));
}).writeDefFile(dir);

// Allpass — same param mapping as Comb
SynthDef(\imbolc_allpass, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=440, resonance=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var finalRes = (resonance.lag(lag) + resMod).clip(0, 1);
    var delay = finalCutoff.reciprocal.clip(0.00005, 0.05);
    var decayTime = finalRes.linlin(0, 1, 0.01, 2.0);
    Out.ar(out, AllpassC.ar(sig, 0.05, delay, decayTime));
}).writeDefFile(dir);

// Vowel — morphs A/E/I/O/U formants via shape param
SynthDef(\imbolc_vowel, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, shape=0, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var q = (resonance.lag(lag) + resMod).clip(0, 1).linlin(0, 1, 1, 0.05);
    var vowelFreqs = #[800, 400, 270, 450, 325]; // A E I O U
    var idx = shape.lag(lag) * 4;
    var f1 = Select.kr(idx.round, vowelFreqs) * (finalCutoff / 1000);
    Out.ar(out, BPF.ar(sig, f1.clip(80, 12000), q));
}).writeDefFile(dir);

// ResDrive — resonant LPF with input saturation
SynthDef(\imbolc_resdrive, { |in=1024, out=1026, cutoff_mod_in=(-1), res_mod_in=(-1), cutoff=1000, resonance=0.5, drive=1, lag=0.02|
    var sig = In.ar(in, 2);
    var cutoffMod = Select.kr(cutoff_mod_in >= 0, [0, In.kr(cutoff_mod_in)]);
    var resMod = Select.kr(res_mod_in >= 0, [0, In.kr(res_mod_in)]);
    var finalCutoff = (cutoff.lag(lag) * (1 + cutoffMod)).clip(20, 20000);
    var q = (resonance.lag(lag) + resMod).clip(0, 1).linlin(0, 1, 1, 0.05);
    Out.ar(out, RLPF.ar((sig * drive.lag(lag)).tanh, finalCutoff, q));
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
// 12-Band Parametric EQ
// ============================================================================
SynthDef(\imbolc_eq12, { |in=1024, out=1026, lag=0.02,
    b0_freq=40,    b0_gain=0, b0_q=0.7, b0_on=1,
    b1_freq=80,    b1_gain=0, b1_q=1.0, b1_on=1,
    b2_freq=160,   b2_gain=0, b2_q=1.0, b2_on=1,
    b3_freq=320,   b3_gain=0, b3_q=1.0, b3_on=1,
    b4_freq=640,   b4_gain=0, b4_q=1.0, b4_on=1,
    b5_freq=1200,  b5_gain=0, b5_q=1.0, b5_on=1,
    b6_freq=2500,  b6_gain=0, b6_q=1.0, b6_on=1,
    b7_freq=5000,  b7_gain=0, b7_q=1.0, b7_on=1,
    b8_freq=8000,  b8_gain=0, b8_q=1.0, b8_on=1,
    b9_freq=12000, b9_gain=0, b9_q=1.0, b9_on=1,
    b10_freq=16000, b10_gain=0, b10_q=1.0, b10_on=1,
    b11_freq=18000, b11_gain=0, b11_q=0.7, b11_on=1|

    var sig = In.ar(in, 2);
    var b0f, b0g, b0r, b0;
    var b1f, b1g, b1r, b1;
    var b2f, b2g, b2r, b2;
    var b3f, b3g, b3r, b3;
    var b4f, b4g, b4r, b4;
    var b5f, b5g, b5r, b5;
    var b6f, b6g, b6r, b6;
    var b7f, b7g, b7r, b7;
    var b8f, b8g, b8r, b8;
    var b9f, b9g, b9r, b9;
    var b10f, b10g, b10r, b10;
    var b11f, b11g, b11r, b11;

    // Band 0: Low Shelf
    b0f = b0_freq.lag(lag).clip(20, 20000);
    b0g = b0_gain.lag(lag);
    b0r = b0_q.lag(lag).reciprocal.clip(0.1, 10);
    b0 = BLowShelf.ar(sig, b0f, b0r, b0g);
    sig = Select.ar(b0_on, [sig, b0]);

    // Band 1: Peaking
    b1f = b1_freq.lag(lag).clip(20, 20000);
    b1g = b1_gain.lag(lag);
    b1r = b1_q.lag(lag).reciprocal.clip(0.1, 10);
    b1 = BPeakEQ.ar(sig, b1f, b1r, b1g);
    sig = Select.ar(b1_on, [sig, b1]);

    // Band 2: Peaking
    b2f = b2_freq.lag(lag).clip(20, 20000);
    b2g = b2_gain.lag(lag);
    b2r = b2_q.lag(lag).reciprocal.clip(0.1, 10);
    b2 = BPeakEQ.ar(sig, b2f, b2r, b2g);
    sig = Select.ar(b2_on, [sig, b2]);

    // Band 3: Peaking
    b3f = b3_freq.lag(lag).clip(20, 20000);
    b3g = b3_gain.lag(lag);
    b3r = b3_q.lag(lag).reciprocal.clip(0.1, 10);
    b3 = BPeakEQ.ar(sig, b3f, b3r, b3g);
    sig = Select.ar(b3_on, [sig, b3]);

    // Band 4: Peaking
    b4f = b4_freq.lag(lag).clip(20, 20000);
    b4g = b4_gain.lag(lag);
    b4r = b4_q.lag(lag).reciprocal.clip(0.1, 10);
    b4 = BPeakEQ.ar(sig, b4f, b4r, b4g);
    sig = Select.ar(b4_on, [sig, b4]);

    // Band 5: Peaking
    b5f = b5_freq.lag(lag).clip(20, 20000);
    b5g = b5_gain.lag(lag);
    b5r = b5_q.lag(lag).reciprocal.clip(0.1, 10);
    b5 = BPeakEQ.ar(sig, b5f, b5r, b5g);
    sig = Select.ar(b5_on, [sig, b5]);

    // Band 6: Peaking
    b6f = b6_freq.lag(lag).clip(20, 20000);
    b6g = b6_gain.lag(lag);
    b6r = b6_q.lag(lag).reciprocal.clip(0.1, 10);
    b6 = BPeakEQ.ar(sig, b6f, b6r, b6g);
    sig = Select.ar(b6_on, [sig, b6]);

    // Band 7: Peaking
    b7f = b7_freq.lag(lag).clip(20, 20000);
    b7g = b7_gain.lag(lag);
    b7r = b7_q.lag(lag).reciprocal.clip(0.1, 10);
    b7 = BPeakEQ.ar(sig, b7f, b7r, b7g);
    sig = Select.ar(b7_on, [sig, b7]);

    // Band 8: Peaking
    b8f = b8_freq.lag(lag).clip(20, 20000);
    b8g = b8_gain.lag(lag);
    b8r = b8_q.lag(lag).reciprocal.clip(0.1, 10);
    b8 = BPeakEQ.ar(sig, b8f, b8r, b8g);
    sig = Select.ar(b8_on, [sig, b8]);

    // Band 9: Peaking
    b9f = b9_freq.lag(lag).clip(20, 20000);
    b9g = b9_gain.lag(lag);
    b9r = b9_q.lag(lag).reciprocal.clip(0.1, 10);
    b9 = BPeakEQ.ar(sig, b9f, b9r, b9g);
    sig = Select.ar(b9_on, [sig, b9]);

    // Band 10: Peaking
    b10f = b10_freq.lag(lag).clip(20, 20000);
    b10g = b10_gain.lag(lag);
    b10r = b10_q.lag(lag).reciprocal.clip(0.1, 10);
    b10 = BPeakEQ.ar(sig, b10f, b10r, b10g);
    sig = Select.ar(b10_on, [sig, b10]);

    // Band 11: High Shelf
    b11f = b11_freq.lag(lag).clip(20, 20000);
    b11g = b11_gain.lag(lag);
    b11r = b11_q.lag(lag).reciprocal.clip(0.1, 10);
    b11 = BHiShelf.ar(sig, b11f, b11r, b11g);
    sig = Select.ar(b11_on, [sig, b11]);

    Out.ar(out, sig);
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
    var readPos, pitchRatio, trig, wet;
    BufWr.ar(mono, buf, phase);
    readPos = (phase - (time.lag(lag) * SampleRate.ir)).wrap(0, bufFrames);
    pitchRatio = 2 ** (pitch.lag(lag) / 12);
    trig = Impulse.ar(density);
    wet = TGrains.ar(2, trig, buf, pitchRatio, readPos / SampleRate.ir, grain_size.lag(lag), 0, 0.5);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Granular Freeze — Frozen grain position + scatter
SynthDef(\imbolc_granular_freeze, { |in=1024, out=1026, grain_size=0.1, density=10, pitch=0, spread=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var bufFrames = 44100 * 2;
    var buf = LocalBuf(bufFrames, 1);
    var phase = Phasor.ar(0, 1, 0, bufFrames);
    var pitchRatio, trig, scatter, grainPos, wet;
    BufWr.ar(mono, buf, phase);
    pitchRatio = 2 ** (pitch.lag(lag) / 12);
    trig = Impulse.ar(density);
    // Scatter grain position around current write position
    scatter = TRand.ar(0, spread.lag(lag) * bufFrames, trig);
    grainPos = (phase - scatter).wrap(0, bufFrames) / SampleRate.ir;
    wet = TGrains.ar(2, trig, buf, pitchRatio, grainPos, grain_size.lag(lag), 0, 0.5);
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
// New Effects - Vocoder, Ring Mod, Autopan, Resonator, Multiband Comp,
// Para EQ, Spectral Freeze, Glitch, Leslie, Spring Reverb,
// Env Follower, Mid/Side, Crossfader
// ============================================================================

// Vocoder — analysis bands on input, resynthesis with oscillator bank
SynthDef(\imbolc_vocoder, { |in=1024, out=1026, bands=16, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var wet = Mix.fill(16, { |i|
        var active = (i < bands);
        var freq = (100 * (2 ** (i * 0.5))).min(16000);
        var analysis = Amplitude.kr(BPF.ar(mono, freq, 0.1), 0.01, 0.05);
        SinOsc.ar(freq) * analysis * active;
    });
    wet = [wet, wet];
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Ring Modulator
SynthDef(\imbolc_ringmod, { |in=1024, out=1026, freq=440, depth=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mod = SinOsc.ar(freq.lag(lag));
    var wet = sig * ((1 - depth.lag(lag)) + (mod * depth.lag(lag)));
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Autopan — LFO-driven stereo panning (shape: 0=sine, 1=square, 2=tri)
SynthDef(\imbolc_autopan, { |in=1024, out=1026, rate=2, depth=0.5, shape=0, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var lfo = Select.kr(shape, [SinOsc.kr(rate), LFPulse.kr(rate).range(-1,1), LFTri.kr(rate)]) * depth.lag(lag);
    var wet = Pan2.ar(mono, lfo);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Resonator — tuned CombC feedback
SynthDef(\imbolc_resonator, { |in=1024, out=1026, freq=440, decay=1.0, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var delay = freq.lag(lag).reciprocal.clip(0.00005, 0.05);
    var wet = CombC.ar(sig, 0.05, delay, decay.lag(lag));
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Multiband Compressor — 3-band split + independent Compander
SynthDef(\imbolc_multiband_comp, { |in=1024, out=1026, low_thresh=0.3, mid_thresh=0.3, hi_thresh=0.3, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var lo = LPF.ar(sig, 250);
    var mid = BPF.ar(sig, 2000, 1);
    var hi = HPF.ar(sig, 6000);
    var wet = Compander.ar(lo, lo, low_thresh.lag(lag), 1, 0.25, 0.01, 0.1)
            + Compander.ar(mid, mid, mid_thresh.lag(lag), 1, 0.25, 0.01, 0.1)
            + Compander.ar(hi, hi, hi_thresh.lag(lag), 1, 0.25, 0.01, 0.1);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Parametric EQ — 3-band BPeakEQ
SynthDef(\imbolc_para_eq, { |in=1024, out=1026,
    lo_freq=200, lo_gain=0, mid_freq=1000, mid_gain=0, hi_freq=5000, hi_gain=0, lag=0.02|
    var sig = In.ar(in, 2);
    sig = BPeakEQ.ar(sig, lo_freq.lag(lag).clip(20, 20000), 1, lo_gain.lag(lag));
    sig = BPeakEQ.ar(sig, mid_freq.lag(lag).clip(20, 20000), 1, mid_gain.lag(lag));
    sig = BPeakEQ.ar(sig, hi_freq.lag(lag).clip(20, 20000), 1, hi_gain.lag(lag));
    Out.ar(out, sig);
}).writeDefFile(dir);

// Spectral Freeze — FFT freeze + blur
SynthDef(\imbolc_spectral_freeze, { |in=1024, out=1026, freeze=0, blur=0, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var chain = FFT(LocalBuf(2048), mono);
    var wet;
    chain = PV_MagFreeze(chain, freeze.lag(lag) > 0.5);
    chain = PV_MagSmear(chain, (blur.lag(lag) * 20).asInteger);
    wet = IFFT(chain) ! 2;
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Glitch — buffer capture with triggered stutter replay
SynthDef(\imbolc_glitch, { |in=1024, out=1026, rate=4, size=0.1, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var buf = LocalBuf(44100, 1);
    var phase = Phasor.ar(0, 1, 0, 44100);
    var trig = Impulse.ar(rate);
    var readPhase = Latch.ar(phase, trig);
    var wet = BufRd.ar(1, buf,
        Phasor.ar(trig, 1, readPhase, readPhase + (size.lag(lag) * SampleRate.ir)), loop: 1) ! 2;
    BufWr.ar(mono, buf, phase);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Leslie — rotary speaker (horn doppler + drum)
SynthDef(\imbolc_leslie, { |in=1024, out=1026, speed=1, horn_depth=0.5, mix=0.5, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var hornRate = speed.lag(lag).linlin(0, 2, 0.5, 8);
    var hornMod = SinOsc.kr(hornRate) * horn_depth.lag(lag);
    var horn = DelayL.ar(mono, 0.01, (0.003 + (hornMod * 0.002)).clip(0.0001, 0.01));
    var hornAM = horn * (1 + (SinOsc.kr(hornRate, 0.5pi) * horn_depth.lag(lag) * 0.3));
    var drumRate = hornRate * 0.6;
    var drumMod = SinOsc.kr(drumRate);
    var drum = DelayL.ar(mono, 0.01, (0.004 + (drumMod * 0.001)).clip(0.0001, 0.01));
    var wet = [(hornAM * 0.7) + (drum * 0.3), (DelayL.ar(hornAM, 0.001, 0.0003) * 0.7) + (drum * 0.3)];
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Spring Reverb — allpass diffusion + comb bank
SynthDef(\imbolc_spring_reverb, { |in=1024, out=1026, decay=2.0, tone=0.5, mix=0.3, lag=0.02|
    var sig = In.ar(in, 2);
    var mono = (sig[0] + sig[1]) * 0.5;
    var input = AllpassN.ar(AllpassN.ar(mono, 0.05, 0.017, 0.1), 0.05, 0.023, 0.1);
    var wet = Mix([
        CombL.ar(input, 0.2, 0.0397, decay.lag(lag)),
        CombL.ar(input, 0.2, 0.0531, decay.lag(lag) * 0.9),
        CombL.ar(input, 0.2, 0.0673, decay.lag(lag) * 0.8),
        CombL.ar(input, 0.2, 0.0789, decay.lag(lag) * 0.7),
    ]) * 0.25;
    wet = LPF.ar(wet, tone.lag(lag).linexp(0, 1, 500, 8000));
    wet = [wet, DelayL.ar(wet, 0.01, 0.003)];
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (wet * mix.lag(lag)));
}).writeDefFile(dir);

// Envelope Follower — amplitude-shaping
SynthDef(\imbolc_env_follower, { |in=1024, out=1026, attack=0.01, release=0.1, depth=0.5, mix=1.0, lag=0.02|
    var sig = In.ar(in, 2);
    var follower = Amplitude.kr((sig[0] + sig[1]) * 0.5, attack.lag(lag), release.lag(lag));
    var shaped = sig * follower.linlin(0, 1, 1 - depth.lag(lag), 1);
    Out.ar(out, (sig * (1 - mix.lag(lag))) + (shaped * mix.lag(lag)));
}).writeDefFile(dir);

// Mid/Side — width + independent mid/side gain
SynthDef(\imbolc_midside, { |in=1024, out=1026, width=1.0, mid_gain=0, side_gain=0, lag=0.02|
    var sig = In.ar(in, 2);
    var mid = (sig[0] + sig[1]) * 0.5 * (mid_gain.lag(lag) / 6).dbamp;
    var side = (sig[0] - sig[1]) * 0.5 * (side_gain.lag(lag) / 6).dbamp * width.lag(lag);
    Out.ar(out, [mid + side, mid - side]);
}).writeDefFile(dir);

// Crossfader — A/B crossfade between input and secondary bus
SynthDef(\imbolc_crossfader, { |in=1024, out=1026, crossfade=0.5, bus_b=0, lag=0.02|
    var sigA = In.ar(in, 2);
    var sigB = Select.ar(bus_b > 0, [DC.ar(0) ! 2, In.ar(bus_b, 2)]);
    var cf = crossfade.lag(lag);
    Out.ar(out, (sigA * (1 - cf)) + (sigB * cf));
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
