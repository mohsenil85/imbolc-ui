use serde::{Deserialize, Serialize};

pub type ModuleId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortType {
    Audio,
    Control,
    Gate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortDef {
    pub name: &'static str,
    pub port_type: PortType,
    pub direction: PortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleType {
    SawOsc,
    SinOsc,
    SqrOsc,
    TriOsc, // Oscillators
    Lpf,
    Hpf,
    Bpf, // Filters
    AdsrEnv, // Envelopes
    Lfo,     // Modulation
    Delay,
    Reverb, // Effects
    Output, // Output
}

impl ModuleType {
    pub fn name(&self) -> &'static str {
        match self {
            ModuleType::SawOsc => "Saw Oscillator",
            ModuleType::SinOsc => "Sine Oscillator",
            ModuleType::SqrOsc => "Square Oscillator",
            ModuleType::TriOsc => "Triangle Oscillator",
            ModuleType::Lpf => "Low-Pass Filter",
            ModuleType::Hpf => "High-Pass Filter",
            ModuleType::Bpf => "Band-Pass Filter",
            ModuleType::AdsrEnv => "ADSR Envelope",
            ModuleType::Lfo => "LFO",
            ModuleType::Delay => "Delay",
            ModuleType::Reverb => "Reverb",
            ModuleType::Output => "Output",
        }
    }

    pub fn default_params(&self) -> Vec<Param> {
        match self {
            ModuleType::SawOsc | ModuleType::SinOsc | ModuleType::SqrOsc | ModuleType::TriOsc => {
                vec![
                    Param {
                        name: "freq".to_string(),
                        value: ParamValue::Float(440.0),
                        min: 20.0,
                        max: 20000.0,
                    },
                    Param {
                        name: "amp".to_string(),
                        value: ParamValue::Float(0.5),
                        min: 0.0,
                        max: 1.0,
                    },
                ]
            }
            ModuleType::Lpf | ModuleType::Hpf | ModuleType::Bpf => vec![
                Param {
                    name: "cutoff".to_string(),
                    value: ParamValue::Float(1000.0),
                    min: 20.0,
                    max: 20000.0,
                },
                Param {
                    name: "resonance".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
            ],
            ModuleType::AdsrEnv => vec![
                Param {
                    name: "attack".to_string(),
                    value: ParamValue::Float(0.01),
                    min: 0.0,
                    max: 5.0,
                },
                Param {
                    name: "decay".to_string(),
                    value: ParamValue::Float(0.1),
                    min: 0.0,
                    max: 5.0,
                },
                Param {
                    name: "sustain".to_string(),
                    value: ParamValue::Float(0.7),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "release".to_string(),
                    value: ParamValue::Float(0.3),
                    min: 0.0,
                    max: 10.0,
                },
            ],
            ModuleType::Lfo => vec![
                Param {
                    name: "rate".to_string(),
                    value: ParamValue::Float(1.0),
                    min: 0.01,
                    max: 100.0,
                },
                Param {
                    name: "depth".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
            ],
            ModuleType::Delay => vec![
                Param {
                    name: "time".to_string(),
                    value: ParamValue::Float(0.3),
                    min: 0.0,
                    max: 2.0,
                },
                Param {
                    name: "feedback".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "mix".to_string(),
                    value: ParamValue::Float(0.3),
                    min: 0.0,
                    max: 1.0,
                },
            ],
            ModuleType::Reverb => vec![
                Param {
                    name: "room".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "damp".to_string(),
                    value: ParamValue::Float(0.5),
                    min: 0.0,
                    max: 1.0,
                },
                Param {
                    name: "mix".to_string(),
                    value: ParamValue::Float(0.3),
                    min: 0.0,
                    max: 1.0,
                },
            ],
            ModuleType::Output => vec![Param {
                name: "gain".to_string(),
                value: ParamValue::Float(1.0),
                min: 0.0,
                max: 2.0,
            }],
        }
    }

    pub fn all_types() -> Vec<ModuleType> {
        vec![
            ModuleType::SawOsc,
            ModuleType::SinOsc,
            ModuleType::SqrOsc,
            ModuleType::TriOsc,
            ModuleType::Lpf,
            ModuleType::Hpf,
            ModuleType::Bpf,
            ModuleType::AdsrEnv,
            ModuleType::Lfo,
            ModuleType::Delay,
            ModuleType::Reverb,
            ModuleType::Output,
        ]
    }

    fn short_name(&self) -> &'static str {
        match self {
            ModuleType::SawOsc => "saw",
            ModuleType::SinOsc => "sin",
            ModuleType::SqrOsc => "sqr",
            ModuleType::TriOsc => "tri",
            ModuleType::Lpf => "lpf",
            ModuleType::Hpf => "hpf",
            ModuleType::Bpf => "bpf",
            ModuleType::AdsrEnv => "adsr",
            ModuleType::Lfo => "lfo",
            ModuleType::Delay => "delay",
            ModuleType::Reverb => "reverb",
            ModuleType::Output => "output",
        }
    }

    pub fn ports(&self) -> Vec<PortDef> {
        match self {
            // Oscillators: audio output only
            ModuleType::SawOsc | ModuleType::SinOsc | ModuleType::SqrOsc | ModuleType::TriOsc => {
                vec![PortDef {
                    name: "out",
                    port_type: PortType::Audio,
                    direction: PortDirection::Output,
                }]
            }
            // Filters: audio in/out + control mod input
            ModuleType::Lpf | ModuleType::Hpf | ModuleType::Bpf => vec![
                PortDef {
                    name: "in",
                    port_type: PortType::Audio,
                    direction: PortDirection::Input,
                },
                PortDef {
                    name: "out",
                    port_type: PortType::Audio,
                    direction: PortDirection::Output,
                },
                PortDef {
                    name: "cutoff_mod",
                    port_type: PortType::Control,
                    direction: PortDirection::Input,
                },
            ],
            // ADSR Envelope: gate input, control output
            ModuleType::AdsrEnv => vec![
                PortDef {
                    name: "gate",
                    port_type: PortType::Gate,
                    direction: PortDirection::Input,
                },
                PortDef {
                    name: "out",
                    port_type: PortType::Control,
                    direction: PortDirection::Output,
                },
            ],
            // LFO: control output only
            ModuleType::Lfo => vec![PortDef {
                name: "out",
                port_type: PortType::Control,
                direction: PortDirection::Output,
            }],
            // Effects: audio in/out
            ModuleType::Delay | ModuleType::Reverb => vec![
                PortDef {
                    name: "in",
                    port_type: PortType::Audio,
                    direction: PortDirection::Input,
                },
                PortDef {
                    name: "out",
                    port_type: PortType::Audio,
                    direction: PortDirection::Output,
                },
            ],
            // Output: terminal node with audio input only
            ModuleType::Output => vec![PortDef {
                name: "in",
                port_type: PortType::Audio,
                direction: PortDirection::Input,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub value: ParamValue,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: ModuleId,
    pub module_type: ModuleType,
    pub name: String,
    pub params: Vec<Param>,
}

impl Module {
    pub fn new(id: ModuleId, module_type: ModuleType) -> Self {
        let params = module_type.default_params();
        let name = format!("{}-{}", module_type.short_name(), id);

        Self {
            id,
            module_type,
            name,
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_creation() {
        let module = Module::new(1, ModuleType::SawOsc);
        assert_eq!(module.id, 1);
        assert_eq!(module.module_type, ModuleType::SawOsc);
        assert_eq!(module.name, "saw-1");
        assert_eq!(module.params.len(), 2); // freq and amp
    }

    #[test]
    fn test_oscillator_default_params() {
        let module = Module::new(0, ModuleType::SinOsc);
        assert_eq!(module.params.len(), 2);
        assert_eq!(module.params[0].name, "freq");
        assert_eq!(module.params[1].name, "amp");

        if let ParamValue::Float(freq) = module.params[0].value {
            assert_eq!(freq, 440.0);
        } else {
            panic!("Expected Float value for freq");
        }

        if let ParamValue::Float(amp) = module.params[1].value {
            assert_eq!(amp, 0.5);
        } else {
            panic!("Expected Float value for amp");
        }
    }

    #[test]
    fn test_filter_default_params() {
        let module = Module::new(0, ModuleType::Lpf);
        assert_eq!(module.params.len(), 2);
        assert_eq!(module.params[0].name, "cutoff");
        assert_eq!(module.params[1].name, "resonance");

        if let ParamValue::Float(cutoff) = module.params[0].value {
            assert_eq!(cutoff, 1000.0);
        } else {
            panic!("Expected Float value for cutoff");
        }
    }

    #[test]
    fn test_adsr_default_params() {
        let module = Module::new(0, ModuleType::AdsrEnv);
        assert_eq!(module.params.len(), 4);
        assert_eq!(module.params[0].name, "attack");
        assert_eq!(module.params[1].name, "decay");
        assert_eq!(module.params[2].name, "sustain");
        assert_eq!(module.params[3].name, "release");
    }

    #[test]
    fn test_all_module_types() {
        let types = ModuleType::all_types();
        assert_eq!(types.len(), 12);
        assert!(types.contains(&ModuleType::SawOsc));
        assert!(types.contains(&ModuleType::Output));
    }

    #[test]
    fn test_module_type_names() {
        assert_eq!(ModuleType::SawOsc.name(), "Saw Oscillator");
        assert_eq!(ModuleType::Lpf.name(), "Low-Pass Filter");
        assert_eq!(ModuleType::AdsrEnv.name(), "ADSR Envelope");
    }

    #[test]
    fn test_module_naming() {
        let module1 = Module::new(1, ModuleType::SawOsc);
        let module2 = Module::new(2, ModuleType::Lpf);
        assert_eq!(module1.name, "saw-1");
        assert_eq!(module2.name, "lpf-2");
    }

    #[test]
    fn test_oscillator_has_output_port() {
        let ports = ModuleType::SawOsc.ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "out");
        assert_eq!(ports[0].port_type, PortType::Audio);
        assert_eq!(ports[0].direction, PortDirection::Output);

        // All oscillators should have the same ports
        assert_eq!(ModuleType::SinOsc.ports().len(), 1);
        assert_eq!(ModuleType::SqrOsc.ports().len(), 1);
        assert_eq!(ModuleType::TriOsc.ports().len(), 1);
    }

    #[test]
    fn test_filter_has_in_out_cutoff_mod() {
        let ports = ModuleType::Lpf.ports();
        assert_eq!(ports.len(), 3);

        let names: Vec<&str> = ports.iter().map(|p| p.name).collect();
        assert!(names.contains(&"in"));
        assert!(names.contains(&"out"));
        assert!(names.contains(&"cutoff_mod"));

        // Check port types
        let in_port = ports.iter().find(|p| p.name == "in").unwrap();
        assert_eq!(in_port.port_type, PortType::Audio);
        assert_eq!(in_port.direction, PortDirection::Input);

        let cutoff_mod = ports.iter().find(|p| p.name == "cutoff_mod").unwrap();
        assert_eq!(cutoff_mod.port_type, PortType::Control);
        assert_eq!(cutoff_mod.direction, PortDirection::Input);

        // All filters should have the same ports
        assert_eq!(ModuleType::Hpf.ports().len(), 3);
        assert_eq!(ModuleType::Bpf.ports().len(), 3);
    }

    #[test]
    fn test_adsr_has_gate_and_output() {
        let ports = ModuleType::AdsrEnv.ports();
        assert_eq!(ports.len(), 2);

        let gate = ports.iter().find(|p| p.name == "gate").unwrap();
        assert_eq!(gate.port_type, PortType::Gate);
        assert_eq!(gate.direction, PortDirection::Input);

        let out = ports.iter().find(|p| p.name == "out").unwrap();
        assert_eq!(out.port_type, PortType::Control);
        assert_eq!(out.direction, PortDirection::Output);
    }

    #[test]
    fn test_lfo_has_control_output() {
        let ports = ModuleType::Lfo.ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "out");
        assert_eq!(ports[0].port_type, PortType::Control);
        assert_eq!(ports[0].direction, PortDirection::Output);
    }

    #[test]
    fn test_effects_have_audio_in_out() {
        for module_type in [ModuleType::Delay, ModuleType::Reverb] {
            let ports = module_type.ports();
            assert_eq!(ports.len(), 2);

            let in_port = ports.iter().find(|p| p.name == "in").unwrap();
            assert_eq!(in_port.port_type, PortType::Audio);
            assert_eq!(in_port.direction, PortDirection::Input);

            let out_port = ports.iter().find(|p| p.name == "out").unwrap();
            assert_eq!(out_port.port_type, PortType::Audio);
            assert_eq!(out_port.direction, PortDirection::Output);
        }
    }

    #[test]
    fn test_output_has_audio_input_only() {
        let ports = ModuleType::Output.ports();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name, "in");
        assert_eq!(ports[0].port_type, PortType::Audio);
        assert_eq!(ports[0].direction, PortDirection::Input);
    }
}
