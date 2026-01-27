use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType};

pub struct OscClient {
    socket: UdpSocket,
    server_addr: String,
}

impl OscClient {
    pub fn new(server_addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket,
            server_addr: server_addr.to_string(),
        })
    }

    pub fn send_message(&self, addr: &str, args: Vec<OscType>) -> std::io::Result<()> {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args,
        });
        let buf = rosc::encoder::encode(&msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.socket.send_to(&buf, &self.server_addr)?;
        Ok(())
    }

    /// /g_new group_id add_action target
    pub fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> std::io::Result<()> {
        self.send_message("/g_new", vec![
            OscType::Int(group_id),
            OscType::Int(add_action),
            OscType::Int(target),
        ])
    }

    /// /s_new synthdef node_id add_action target [param value ...]
    pub fn create_synth(&self, synth_def: &str, node_id: i32, params: &[(String, f32)]) -> std::io::Result<()> {
        let mut args: Vec<OscType> = vec![
            OscType::String(synth_def.to_string()),
            OscType::Int(node_id),
            OscType::Int(1),  // addToTail
            OscType::Int(0),  // default group
        ];
        for (name, value) in params {
            args.push(OscType::String(name.clone()));
            args.push(OscType::Float(*value));
        }
        self.send_message("/s_new", args)
    }

    /// /s_new synthdef node_id addToTail(1) group [param value ...]
    pub fn create_synth_in_group(&self, synth_def: &str, node_id: i32, group_id: i32, params: &[(String, f32)]) -> std::io::Result<()> {
        let mut args: Vec<OscType> = vec![
            OscType::String(synth_def.to_string()),
            OscType::Int(node_id),
            OscType::Int(1),  // addToTail
            OscType::Int(group_id),
        ];
        for (name, value) in params {
            args.push(OscType::String(name.clone()));
            args.push(OscType::Float(*value));
        }
        self.send_message("/s_new", args)
    }

    pub fn free_node(&self, node_id: i32) -> std::io::Result<()> {
        self.send_message("/n_free", vec![OscType::Int(node_id)])
    }

    pub fn set_param(&self, node_id: i32, param: &str, value: f32) -> std::io::Result<()> {
        self.send_message("/n_set", vec![
            OscType::Int(node_id),
            OscType::String(param.to_string()),
            OscType::Float(value),
        ])
    }

    /// Set multiple params on a node atomically via an OSC bundle
    pub fn set_params_bundled(&self, node_id: i32, params: &[(&str, f32)], time: OscTime) -> std::io::Result<()> {
        let mut args: Vec<OscType> = vec![OscType::Int(node_id)];
        for (name, value) in params {
            args.push(OscType::String(name.to_string()));
            args.push(OscType::Float(*value));
        }
        let msg = OscPacket::Message(OscMessage {
            addr: "/n_set".to_string(),
            args,
        });
        let bundle = OscPacket::Bundle(OscBundle {
            timetag: time,
            content: vec![msg],
        });
        let buf = rosc::encoder::encode(&bundle)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.socket.send_to(&buf, &self.server_addr)?;
        Ok(())
    }

    /// Send multiple messages in a single timestamped bundle
    pub fn send_bundle(&self, messages: Vec<OscMessage>, time: OscTime) -> std::io::Result<()> {
        let content = messages.into_iter().map(OscPacket::Message).collect();
        let bundle = OscPacket::Bundle(OscBundle {
            timetag: time,
            content,
        });
        let buf = rosc::encoder::encode(&bundle)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        self.socket.send_to(&buf, &self.server_addr)?;
        Ok(())
    }
}

/// Convert a SystemTime offset (seconds from now) to an OSC timetag.
/// SC uses NTP epoch (1900-01-01), so we add the NTP-Unix offset.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

pub fn osc_time_from_now(offset_secs: f64) -> OscTime {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs_f64() + offset_secs;
    let secs = total_secs as u64 + NTP_UNIX_OFFSET;
    let frac = ((total_secs.fract()) * (u32::MAX as f64)) as u32;
    OscTime { seconds: secs as u32, fractional: frac }
}

/// Immediate timetag (0,1) — execute as soon as received
pub fn osc_time_immediate() -> OscTime {
    OscTime { seconds: 0, fractional: 1 }
}
