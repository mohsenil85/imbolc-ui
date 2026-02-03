use std::collections::{HashMap, VecDeque};
use std::net::UdpSocket;
use std::sync::{Arc, LazyLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType};

/// Maximum number of waveform samples to keep per audio input instrument
const WAVEFORM_BUFFER_SIZE: usize = 100;

/// Maximum scope samples to keep
const SCOPE_BUFFER_SIZE: usize = 200;

/// A single discovered VST parameter from /vst_param OSC reply
#[derive(Debug, Clone)]
pub struct VstParamReply {
    pub index: u32,
    pub value: f32,
    pub display: String,
}

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
    /// SuperCollider average CPU load from /status.reply
    sc_cpu: Arc<RwLock<f32>>,
    /// OSC round-trip latency in milliseconds
    osc_latency_ms: Arc<RwLock<f32>>,
    /// Timestamp when /status was last sent (for latency measurement)
    status_sent_at: Arc<RwLock<Option<Instant>>>,
    /// VST param query replies: nodeID → Vec<VstParamReply>
    vst_params: Arc<RwLock<HashMap<i32, Vec<VstParamReply>>>>,
}

impl AudioMonitor {
    pub fn new() -> Self {
        Self {
            meter_data: Arc::new(RwLock::new((0.0_f32, 0.0_f32))),
            audio_in_waveforms: Arc::new(RwLock::new(HashMap::new())),
            spectrum_data: Arc::new(RwLock::new([0.0; 7])),
            lufs_data: Arc::new(RwLock::new((0.0, 0.0, 0.0, 0.0))),
            scope_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(SCOPE_BUFFER_SIZE))),
            sc_cpu: Arc::new(RwLock::new(0.0)),
            osc_latency_ms: Arc::new(RwLock::new(0.0)),
            status_sent_at: Arc::new(RwLock::new(None)),
            vst_params: Arc::new(RwLock::new(HashMap::new())),
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

    pub fn sc_cpu(&self) -> f32 {
        self.sc_cpu.read().map(|v| *v).unwrap_or(0.0)
    }

    pub fn osc_latency_ms(&self) -> f32 {
        self.osc_latency_ms.read().map(|v| *v).unwrap_or(0.0)
    }

    /// Mark the time /status was sent, for latency measurement
    pub fn mark_status_sent(&self) {
        if let Ok(mut ts) = self.status_sent_at.write() {
            *ts = Some(Instant::now());
        }
    }

    /// Take accumulated VST param replies for a node (clears the entry)
    pub fn take_vst_params(&self, node_id: i32) -> Option<Vec<VstParamReply>> {
        if let Ok(mut map) = self.vst_params.write() {
            map.remove(&node_id)
        } else {
            None
        }
    }

    /// Clear VST param replies for a node (before starting a new query)
    pub fn clear_vst_params(&self, node_id: i32) {
        if let Ok(mut map) = self.vst_params.write() {
            map.remove(&node_id);
        }
    }

    /// Check if any VST param replies have accumulated for a node
    pub fn has_vst_params(&self, node_id: i32) -> bool {
        self.vst_params
            .read()
            .map(|map| map.contains_key(&node_id))
            .unwrap_or(false)
    }

    /// Get the count of accumulated VST param replies for a node
    pub fn vst_param_count(&self, node_id: i32) -> usize {
        self.vst_params
            .read()
            .map(|map| map.get(&node_id).map(|v| v.len()).unwrap_or(0))
            .unwrap_or(0)
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
    sc_cpu: Arc<RwLock<f32>>,
    osc_latency_ms: Arc<RwLock<f32>>,
    status_sent_at: Arc<RwLock<Option<Instant>>>,
    vst_params: Arc<RwLock<HashMap<i32, Vec<VstParamReply>>>>,
    _recv_thread: Option<JoinHandle<()>>,
}

/// Recursively process an OSC packet (handles bundles wrapping messages)
struct OscRefs {
    meter: Arc<RwLock<(f32, f32)>>,
    waveforms: Arc<RwLock<HashMap<u32, VecDeque<f32>>>>,
    spectrum: Arc<RwLock<[f32; 7]>>,
    lufs: Arc<RwLock<(f32, f32, f32, f32)>>,
    scope: Arc<RwLock<VecDeque<f32>>>,
    sc_cpu: Arc<RwLock<f32>>,
    osc_latency_ms: Arc<RwLock<f32>>,
    status_sent_at: Arc<RwLock<Option<Instant>>>,
    vst_params: Arc<RwLock<HashMap<i32, Vec<VstParamReply>>>>,
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
            } else if msg.addr == "/status.reply" && msg.args.len() >= 6 {
                // /status.reply: [unused, ugens, synths, groups, synthdefs, avg_cpu, peak_cpu]
                let avg_cpu = match msg.args.get(5) {
                    Some(OscType::Float(v)) => *v,
                    _ => 0.0,
                };
                if let Ok(mut cpu) = refs.sc_cpu.write() {
                    *cpu = avg_cpu;
                }
                // Calculate round-trip latency from when /status was sent
                if let Ok(mut ts) = refs.status_sent_at.write() {
                    if let Some(sent) = ts.take() {
                        let latency = sent.elapsed().as_secs_f32() * 1000.0;
                        if let Ok(mut lat) = refs.osc_latency_ms.write() {
                            *lat = latency;
                        }
                    }
                }
            } else if msg.addr == "/vst_param" && msg.args.len() >= 5 {
                // VSTPlugin SendNodeReply: /vst_param nodeID replyID index value display_len char0 char1 ...
                let node_id = match msg.args.get(0) {
                    Some(OscType::Int(v)) => *v,
                    Some(OscType::Float(v)) => *v as i32,
                    _ => return,
                };
                // args[1] = replyID (skip)
                let index = match msg.args.get(2) {
                    Some(OscType::Float(v)) => *v as u32,
                    Some(OscType::Int(v)) => *v as u32,
                    _ => return,
                };
                let value = match msg.args.get(3) {
                    Some(OscType::Float(v)) => *v,
                    _ => return,
                };
                // Decode display string from float array: len = args[4], chars = args[5..5+len]
                let display_len = match msg.args.get(4) {
                    Some(OscType::Float(v)) => *v as usize,
                    Some(OscType::Int(v)) => *v as usize,
                    _ => 0,
                };
                let display: String = (0..display_len)
                    .filter_map(|i| {
                        match msg.args.get(5 + i) {
                            Some(OscType::Float(v)) => Some(*v as u8 as char),
                            Some(OscType::Int(v)) => Some(*v as u8 as char),
                            _ => None,
                        }
                    })
                    .collect();
                if let Ok(mut map) = refs.vst_params.write() {
                    map.entry(node_id).or_default().push(VstParamReply {
                        index,
                        value,
                        display,
                    });
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
        let sc_cpu = Arc::clone(&monitor.sc_cpu);
        let osc_latency_ms = Arc::clone(&monitor.osc_latency_ms);
        let status_sent_at = Arc::clone(&monitor.status_sent_at);
        let vst_params = Arc::clone(&monitor.vst_params);

        // Clone socket for receive thread
        let recv_socket = socket.try_clone()?;
        recv_socket.set_read_timeout(Some(Duration::from_millis(50)))?;
        let refs = OscRefs {
            meter: Arc::clone(&meter_data),
            waveforms: Arc::clone(&audio_in_waveforms),
            spectrum: Arc::clone(&spectrum_data),
            lufs: Arc::clone(&lufs_data),
            scope: Arc::clone(&scope_buffer),
            sc_cpu: Arc::clone(&sc_cpu),
            osc_latency_ms: Arc::clone(&osc_latency_ms),
            status_sent_at: Arc::clone(&status_sent_at),
            vst_params: Arc::clone(&vst_params),
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
            sc_cpu,
            osc_latency_ms,
            status_sent_at,
            vst_params,
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
            sc_cpu: Arc::clone(&self.sc_cpu),
            osc_latency_ms: Arc::clone(&self.osc_latency_ms),
            status_sent_at: Arc::clone(&self.status_sent_at),
            vst_params: Arc::clone(&self.vst_params),
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

/// Convert a monotonic offset (seconds from now) to an OSC timetag.
/// SC uses NTP epoch (1900-01-01), so we add the NTP-Unix offset.
/// Uses a monotonic Instant anchor to avoid wall-clock jumps from NTP adjustments.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

/// Anchor pair captured once at init: (monotonic instant, wall-clock time).
/// All timetags are derived from the Instant elapsed since this anchor,
/// using the SystemTime only as the epoch reference.
static CLOCK_ANCHOR: LazyLock<(Instant, f64)> = LazyLock::new(|| {
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    (Instant::now(), wall)
});

pub fn osc_time_from_now(offset_secs: f64) -> OscTime {
    let (anchor_instant, anchor_wall) = &*CLOCK_ANCHOR;
    let elapsed = anchor_instant.elapsed().as_secs_f64();
    let total_secs = anchor_wall + elapsed + offset_secs;
    let secs = total_secs as u64 + NTP_UNIX_OFFSET;
    let frac = ((total_secs.fract()) * (u32::MAX as f64)) as u32;
    OscTime { seconds: secs as u32, fractional: frac }
}

/// Immediate timetag (0,1) — execute as soon as received
#[allow(dead_code)]
pub fn osc_time_immediate() -> OscTime {
    OscTime { seconds: 0, fractional: 1 }
}

