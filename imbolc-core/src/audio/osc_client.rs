use std::collections::{HashMap, VecDeque};
use std::net::UdpSocket;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType};

/// Maximum number of waveform samples to keep per audio input instrument
const WAVEFORM_BUFFER_SIZE: usize = 100;

/// Maximum scope samples to keep
const SCOPE_BUFFER_SIZE: usize = 200;

/// Shared meter + waveform + visualization data accessible from both threads.
#[derive(Clone, Default)]
pub struct AudioMonitor {
    meter_data: Arc<RwLock<(f32, f32)>>,
    audio_in_waveforms: Arc<RwLock<HashMap<u32, VecDeque<f32>>>>,
    /// 7-band spectrum data
    spectrum_data: Arc<RwLock<[f32; 7]>>,
    /// LUFS data: (peak_l, peak_r, rms_l, rms_r)
    lufs_data: Arc<RwLock<(f32, f32, f32, f32)>>,
    /// Oscilloscope ring buffer
    scope_buffer: Arc<RwLock<VecDeque<f32>>>,
}

impl AudioMonitor {
    pub fn new() -> Self {
        Self {
            meter_data: Arc::new(RwLock::new((0.0_f32, 0.0_f32))),
            audio_in_waveforms: Arc::new(RwLock::new(HashMap::new())),
            spectrum_data: Arc::new(RwLock::new([0.0; 7])),
            lufs_data: Arc::new(RwLock::new((0.0, 0.0, 0.0, 0.0))),
            scope_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(SCOPE_BUFFER_SIZE))),
        }
    }

    pub fn meter_peak(&self) -> (f32, f32) {
        self.meter_data
            .read()
            .map(|data| *data)
            .unwrap_or((0.0, 0.0))
    }

    pub fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.audio_in_waveforms
            .read()
            .map(|waveforms| {
                waveforms
                    .get(&instrument_id)
                    .map(|buffer| buffer.iter().copied().collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    pub fn spectrum_bands(&self) -> [f32; 7] {
        self.spectrum_data
            .read()
            .map(|data| *data)
            .unwrap_or([0.0; 7])
    }

    pub fn lufs_data(&self) -> (f32, f32, f32, f32) {
        self.lufs_data
            .read()
            .map(|data| *data)
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    pub fn scope_buffer(&self) -> Vec<f32> {
        self.scope_buffer
            .read()
            .map(|buf| buf.iter().copied().collect())
            .unwrap_or_default()
    }
}

pub struct OscClient {
    socket: UdpSocket,
    server_addr: String,
    meter_data: Arc<RwLock<(f32, f32)>>,
    /// Waveform data per audio input instrument: instrument_id -> ring buffer of peak values
    audio_in_waveforms: Arc<RwLock<HashMap<u32, VecDeque<f32>>>>,
    spectrum_data: Arc<RwLock<[f32; 7]>>,
    lufs_data: Arc<RwLock<(f32, f32, f32, f32)>>,
    scope_buffer: Arc<RwLock<VecDeque<f32>>>,
    _recv_thread: Option<JoinHandle<()>>,
}

pub trait OscClientLike: Send + Sync {
    fn meter_peak(&self) -> (f32, f32);
    fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32>;
    fn send_message(&self, addr: &str, args: Vec<OscType>) -> std::io::Result<()>;
    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> std::io::Result<()>;
    fn create_synth(&self, synth_def: &str, node_id: i32, params: &[(String, f32)]) -> std::io::Result<()>;
    fn create_synth_in_group(
        &self,
        synth_def: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> std::io::Result<()>;
    fn free_node(&self, node_id: i32) -> std::io::Result<()>;
    fn set_param(&self, node_id: i32, param: &str, value: f32) -> std::io::Result<()>;
    fn set_params_bundled(&self, node_id: i32, params: &[(&str, f32)], time: OscTime) -> std::io::Result<()>;
    fn send_bundle(&self, messages: Vec<OscMessage>, time: OscTime) -> std::io::Result<()>;
    fn load_buffer(&self, bufnum: i32, path: &str) -> std::io::Result<()>;
    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> std::io::Result<()>;
    fn free_buffer(&self, bufnum: i32) -> std::io::Result<()>;
    fn open_buffer_for_write(&self, bufnum: i32, path: &str) -> std::io::Result<()>;
    fn close_buffer(&self, bufnum: i32) -> std::io::Result<()>;
    fn query_buffer(&self, bufnum: i32) -> std::io::Result<()>;
    fn send_unit_cmd(&self, node_id: i32, ugen_index: i32, cmd: &str, args: Vec<OscType>) -> std::io::Result<()>;
}

#[cfg(test)]
#[derive(Debug, Default)]
pub struct NullOscClient;

#[cfg(test)]
impl NullOscClient {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
impl OscClientLike for NullOscClient {
    fn meter_peak(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn audio_in_waveform(&self, _instrument_id: u32) -> Vec<f32> {
        Vec::new()
    }

    fn send_message(&self, _addr: &str, _args: Vec<OscType>) -> std::io::Result<()> {
        Ok(())
    }

    fn create_group(&self, _group_id: i32, _add_action: i32, _target: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn create_synth(&self, _synth_def: &str, _node_id: i32, _params: &[(String, f32)]) -> std::io::Result<()> {
        Ok(())
    }

    fn create_synth_in_group(
        &self,
        _synth_def: &str,
        _node_id: i32,
        _group_id: i32,
        _params: &[(String, f32)],
    ) -> std::io::Result<()> {
        Ok(())
    }

    fn free_node(&self, _node_id: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn set_param(&self, _node_id: i32, _param: &str, _value: f32) -> std::io::Result<()> {
        Ok(())
    }

    fn set_params_bundled(&self, _node_id: i32, _params: &[(&str, f32)], _time: OscTime) -> std::io::Result<()> {
        Ok(())
    }

    fn send_bundle(&self, _messages: Vec<OscMessage>, _time: OscTime) -> std::io::Result<()> {
        Ok(())
    }

    fn load_buffer(&self, _bufnum: i32, _path: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn alloc_buffer(&self, _bufnum: i32, _num_frames: i32, _num_channels: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn free_buffer(&self, _bufnum: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn open_buffer_for_write(&self, _bufnum: i32, _path: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn close_buffer(&self, _bufnum: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn query_buffer(&self, _bufnum: i32) -> std::io::Result<()> {
        Ok(())
    }

    fn send_unit_cmd(&self, _node_id: i32, _ugen_index: i32, _cmd: &str, _args: Vec<OscType>) -> std::io::Result<()> {
        Ok(())
    }
}

/// Recursively process an OSC packet (handles bundles wrapping messages)
struct OscRefs {
    meter: Arc<RwLock<(f32, f32)>>,
    waveforms: Arc<RwLock<HashMap<u32, VecDeque<f32>>>>,
    spectrum: Arc<RwLock<[f32; 7]>>,
    lufs: Arc<RwLock<(f32, f32, f32, f32)>>,
    scope: Arc<RwLock<VecDeque<f32>>>,
}

fn handle_osc_packet(packet: &OscPacket, refs: &OscRefs) {
    match packet {
        OscPacket::Message(msg) => {
            if msg.addr == "/meter" && msg.args.len() >= 6 {
                let peak_l = match msg.args.get(2) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                let peak_r = match msg.args.get(4) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                if let Ok(mut data) = refs.meter.write() {
                    *data = (peak_l, peak_r);
                }
            } else if msg.addr == "/audio_in_level" && msg.args.len() >= 4 {
                // SendPeakRMS format: /audio_in_level nodeID replyID peakL rmsL peakR rmsR
                let instrument_id = match msg.args.get(1) {
                    Some(OscType::Int(v)) => *v as u32,
                    Some(OscType::Float(v)) => *v as u32,
                    _ => return,
                };
                let peak = match msg.args.get(2) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                if let Ok(mut waveforms) = refs.waveforms.write() {
                    let buffer = waveforms.entry(instrument_id).or_insert_with(VecDeque::new);
                    buffer.push_back(peak);
                    while buffer.len() > WAVEFORM_BUFFER_SIZE {
                        buffer.pop_front();
                    }
                }
            } else if msg.addr == "/spectrum" && msg.args.len() >= 9 {
                // SendReply format: /spectrum nodeID replyID val0 val1 ... val6
                let mut bands = [0.0_f32; 7];
                for i in 0..7 {
                    bands[i] = match msg.args.get(2 + i) {
                        Some(OscType::Float(v)) => *v,
                        _ => 0.0,
                    };
                }
                if let Ok(mut data) = refs.spectrum.write() {
                    *data = bands;
                }
            } else if msg.addr == "/lufs" && msg.args.len() >= 6 {
                // SendPeakRMS format: /lufs nodeID replyID peakL rmsL peakR rmsR
                let peak_l = match msg.args.get(2) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                let rms_l = match msg.args.get(3) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                let peak_r = match msg.args.get(4) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                let rms_r = match msg.args.get(5) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                if let Ok(mut data) = refs.lufs.write() {
                    *data = (peak_l, peak_r, rms_l, rms_r);
                }
            } else if msg.addr == "/scope" && msg.args.len() >= 3 {
                // SendReply format: /scope nodeID replyID peakValue
                let peak = match msg.args.get(2) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                if let Ok(mut buf) = refs.scope.write() {
                    buf.push_back(peak);
                    while buf.len() > SCOPE_BUFFER_SIZE {
                        buf.pop_front();
                    }
                }
            }
        }
        OscPacket::Bundle(bundle) => {
            for p in &bundle.content {
                handle_osc_packet(p, refs);
            }
        }
    }
}

impl OscClient {
    pub fn new(server_addr: &str) -> std::io::Result<Self> {
        let monitor = AudioMonitor::new();
        Self::new_with_monitor(server_addr, monitor)
    }

    pub fn new_with_monitor(server_addr: &str, monitor: AudioMonitor) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let meter_data = Arc::clone(&monitor.meter_data);
        let audio_in_waveforms = Arc::clone(&monitor.audio_in_waveforms);
        let spectrum_data = Arc::clone(&monitor.spectrum_data);
        let lufs_data = Arc::clone(&monitor.lufs_data);
        let scope_buffer = Arc::clone(&monitor.scope_buffer);

        // Clone socket for receive thread
        let recv_socket = socket.try_clone()?;
        recv_socket.set_read_timeout(Some(Duration::from_millis(50)))?;
        let refs = OscRefs {
            meter: Arc::clone(&meter_data),
            waveforms: Arc::clone(&audio_in_waveforms),
            spectrum: Arc::clone(&spectrum_data),
            lufs: Arc::clone(&lufs_data),
            scope: Arc::clone(&scope_buffer),
        };

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match recv_socket.recv(&mut buf) {
                    Ok(n) => {
                        if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) {
                            handle_osc_packet(&packet, &refs);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            socket,
            server_addr: server_addr.to_string(),
            meter_data,
            audio_in_waveforms,
            spectrum_data,
            lufs_data,
            scope_buffer,
            _recv_thread: Some(handle),
        })
    }

    #[allow(dead_code)]
    pub fn monitor(&self) -> AudioMonitor {
        AudioMonitor {
            meter_data: Arc::clone(&self.meter_data),
            audio_in_waveforms: Arc::clone(&self.audio_in_waveforms),
            spectrum_data: Arc::clone(&self.spectrum_data),
            lufs_data: Arc::clone(&self.lufs_data),
            scope_buffer: Arc::clone(&self.scope_buffer),
        }
    }

    /// Get current peak levels (left, right) from the meter synth
    pub fn meter_peak(&self) -> (f32, f32) {
        self.meter_data.read().map(|d| *d).unwrap_or((0.0, 0.0))
    }

    /// Get waveform data for an audio input instrument (returns a copy of the buffer)
    pub fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.audio_in_waveforms
            .read()
            .map(|w| w.get(&instrument_id).map(|d| d.iter().copied().collect()).unwrap_or_default())
            .unwrap_or_default()
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
    #[allow(dead_code)]
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

    /// /b_allocRead bufnum path startFrame numFrames
    /// Load a sound file into a buffer (SuperCollider reads the file)
    #[allow(dead_code)]
    pub fn load_buffer(&self, bufnum: i32, path: &str) -> std::io::Result<()> {
        self.send_message("/b_allocRead", vec![
            OscType::Int(bufnum),
            OscType::String(path.to_string()),
            OscType::Int(0),  // start frame
            OscType::Int(0),  // 0 = read entire file
        ])
    }

    /// /b_alloc bufnum numFrames numChannels
    /// Allocate an empty buffer
    #[allow(dead_code)]
    pub fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> std::io::Result<()> {
        self.send_message("/b_alloc", vec![
            OscType::Int(bufnum),
            OscType::Int(num_frames),
            OscType::Int(num_channels),
        ])
    }

    /// /b_free bufnum
    /// Free a buffer
    pub fn free_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.send_message("/b_free", vec![OscType::Int(bufnum)])
    }

    /// /b_write bufnum path headerFormat sampleFormat numFrames startFrame leaveOpen
    /// Open a buffer for disk writing (WAV, 32-bit float, leave open for streaming)
    pub fn open_buffer_for_write(&self, bufnum: i32, path: &str) -> std::io::Result<()> {
        self.send_message("/b_write", vec![
            OscType::Int(bufnum),
            OscType::String(path.to_string()),
            OscType::String("wav".to_string()),
            OscType::String("float".to_string()),
            OscType::Int(0),  // numFrames (0 = all)
            OscType::Int(0),  // startFrame
            OscType::Int(1),  // leaveOpen = 1
        ])
    }

    /// /b_close bufnum
    /// Close a buffer's soundfile (after DiskOut recording)
    pub fn close_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.send_message("/b_close", vec![OscType::Int(bufnum)])
    }

    /// /b_query bufnum
    /// Query buffer info (results come back asynchronously via /b_info)
    #[allow(dead_code)]
    pub fn query_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.send_message("/b_query", vec![OscType::Int(bufnum)])
    }

    /// /u_cmd nodeID ugenIndex command [args...]
    /// Send a unit command to a specific UGen instance within a synth node.
    /// Used for VSTPlugin UGen commands like /open, /midi_msg, /set, etc.
    pub fn send_unit_cmd(&self, node_id: i32, ugen_index: i32, cmd: &str, args: Vec<OscType>) -> std::io::Result<()> {
        let mut msg_args = vec![
            OscType::Int(node_id),
            OscType::Int(ugen_index),
            OscType::String(cmd.to_string()),
        ];
        msg_args.extend(args);
        self.send_message("/u_cmd", msg_args)
    }
}

impl OscClientLike for OscClient {
    fn meter_peak(&self) -> (f32, f32) {
        self.meter_peak()
    }

    fn audio_in_waveform(&self, instrument_id: u32) -> Vec<f32> {
        self.audio_in_waveform(instrument_id)
    }

    fn send_message(&self, addr: &str, args: Vec<OscType>) -> std::io::Result<()> {
        self.send_message(addr, args)
    }

    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> std::io::Result<()> {
        self.create_group(group_id, add_action, target)
    }

    fn create_synth(&self, synth_def: &str, node_id: i32, params: &[(String, f32)]) -> std::io::Result<()> {
        self.create_synth(synth_def, node_id, params)
    }

    fn create_synth_in_group(
        &self,
        synth_def: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> std::io::Result<()> {
        self.create_synth_in_group(synth_def, node_id, group_id, params)
    }

    fn free_node(&self, node_id: i32) -> std::io::Result<()> {
        self.free_node(node_id)
    }

    fn set_param(&self, node_id: i32, param: &str, value: f32) -> std::io::Result<()> {
        self.set_param(node_id, param, value)
    }

    fn set_params_bundled(&self, node_id: i32, params: &[(&str, f32)], time: OscTime) -> std::io::Result<()> {
        self.set_params_bundled(node_id, params, time)
    }

    fn send_bundle(&self, messages: Vec<OscMessage>, time: OscTime) -> std::io::Result<()> {
        self.send_bundle(messages, time)
    }

    fn load_buffer(&self, bufnum: i32, path: &str) -> std::io::Result<()> {
        self.load_buffer(bufnum, path)
    }

    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> std::io::Result<()> {
        self.alloc_buffer(bufnum, num_frames, num_channels)
    }

    fn free_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.free_buffer(bufnum)
    }

    fn open_buffer_for_write(&self, bufnum: i32, path: &str) -> std::io::Result<()> {
        self.open_buffer_for_write(bufnum, path)
    }

    fn close_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.close_buffer(bufnum)
    }

    fn query_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        self.query_buffer(bufnum)
    }

    fn send_unit_cmd(&self, node_id: i32, ugen_index: i32, cmd: &str, args: Vec<OscType>) -> std::io::Result<()> {
        self.send_unit_cmd(node_id, ugen_index, cmd, args)
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
#[allow(dead_code)]
pub fn osc_time_immediate() -> OscTime {
    OscTime { seconds: 0, fractional: 1 }
}
