// VST Plugin wrapper SynthDefs for ilex
// Requires VSTPlugin extension: Quarks.install("VSTPlugin")
// Run in SuperCollider to generate .scsyndef files
(
var dir = thisProcess.nowExecutingPath.dirname;

// ============================================================================
// VSTi wrapper — persistent source, no envelope
// The VSTPlugin UGen handles its own polyphony via MIDI messages.
// Audio output goes to the instrument's source bus.
// ============================================================================
SynthDef(\ilex_vst_instrument, { |out=1024|
    var sig = VSTPlugin.ar(nil, 2, id: \vst);
    Out.ar(out, sig);
}).writeDefFile(dir);

// ============================================================================
// VST effect wrapper — processes audio through a VST effect plugin
// Reads from input bus, processes through VSTPlugin, writes to output bus.
// ============================================================================
SynthDef(\ilex_vst_effect, { |in=1024, out=1026|
    var sig = In.ar(in, 2);
    sig = VSTPlugin.ar(sig, 2, id: \vst);
    Out.ar(out, sig);
}).writeDefFile(dir);

"VST SynthDefs written to: ".post; dir.postln;
)
