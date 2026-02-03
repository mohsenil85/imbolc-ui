//! Audio backend trait: a semantic-level abstraction over audio server operations.
//!
//! `AudioBackend` captures what the engine *means* to do (create a synth, free a node,
//! set a parameter) independently of how it's done (OSC messages to SuperCollider).
//! This enables unit testing of routing logic without a running audio server.
//!
//! Layers:
//! - `OscClientLike` (osc_client.rs) — transport: how to send/receive OSC packets
//! - `AudioBackend` (this file) — semantic: what operations the engine performs

use std::fmt;
use std::path::Path;

/// Result type for backend operations.
pub type BackendResult<T = ()> = Result<T, BackendError>;

/// Error from a backend operation.
#[derive(Debug, Clone)]
pub struct BackendError(pub String);

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BackendError {}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        BackendError(e.to_string())
    }
}

impl From<String> for BackendError {
    fn from(s: String) -> Self {
        BackendError(s)
    }
}

/// Semantic-level audio backend trait.
///
/// Each method represents a meaningful audio operation. Implementations
/// translate these into server-specific commands (e.g., OSC for SuperCollider)
/// or record them for testing.
pub trait AudioBackend: Send {
    /// Create a group node for execution ordering.
    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> BackendResult;

    /// Create a synth in a specific group with named parameters.
    fn create_synth(
        &self,
        def_name: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> BackendResult;

    /// Free (remove) a node from the server.
    fn free_node(&self, node_id: i32) -> BackendResult;

    /// Set a single parameter on a node.
    fn set_param(&self, node_id: i32, param: &str, value: f32) -> BackendResult;

    /// Set multiple parameters on a node atomically.
    fn set_params(&self, node_id: i32, params: &[(&str, f32)]) -> BackendResult;

    /// Load a sound file into a buffer at the given buffer number.
    fn load_buffer(&self, bufnum: i32, path: &Path) -> BackendResult;

    /// Free a buffer.
    fn free_buffer(&self, bufnum: i32) -> BackendResult;

    /// Allocate an empty buffer with the given frame count and channel count.
    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> BackendResult;

    /// Send a raw message (escape hatch for operations not covered by typed methods).
    fn send_raw(&self, addr: &str, args: Vec<RawArg>) -> BackendResult;
}

/// A loosely-typed argument for `send_raw`, so backends don't depend on `rosc`.
#[derive(Debug, Clone, PartialEq)]
pub enum RawArg {
    Int(i32),
    Float(f32),
    Str(String),
    Blob(Vec<u8>),
}

// ─── SuperCollider Backend ──────────────────────────────────────────

use super::super::osc_client::OscClientLike;

/// Backend implementation that delegates to an `OscClientLike` transport
/// (the existing SuperCollider OSC abstraction).
pub struct ScBackend {
    client: Box<dyn OscClientLike>,
}

impl ScBackend {
    pub fn new(client: Box<dyn OscClientLike>) -> Self {
        Self { client }
    }

    /// Access the underlying OscClientLike for operations not yet covered
    /// by the AudioBackend trait (e.g., send_bundle, send_unit_cmd).
    pub fn osc_client(&self) -> &dyn OscClientLike {
        self.client.as_ref()
    }
}

impl AudioBackend for ScBackend {
    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> BackendResult {
        self.client
            .create_group(group_id, add_action, target)
            .map_err(BackendError::from)
    }

    fn create_synth(
        &self,
        def_name: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> BackendResult {
        self.client
            .create_synth_in_group(def_name, node_id, group_id, params)
            .map_err(BackendError::from)
    }

    fn free_node(&self, node_id: i32) -> BackendResult {
        self.client.free_node(node_id).map_err(BackendError::from)
    }

    fn set_param(&self, node_id: i32, param: &str, value: f32) -> BackendResult {
        self.client
            .set_param(node_id, param, value)
            .map_err(BackendError::from)
    }

    fn set_params(&self, node_id: i32, params: &[(&str, f32)]) -> BackendResult {
        // Set each param individually (OscClientLike doesn't have a batch set without a timetag)
        for &(param, value) in params {
            self.client
                .set_param(node_id, param, value)
                .map_err(BackendError::from)?;
        }
        Ok(())
    }

    fn load_buffer(&self, bufnum: i32, path: &Path) -> BackendResult {
        let path_str = path.to_string_lossy();
        self.client
            .load_buffer(bufnum, &path_str)
            .map_err(BackendError::from)
    }

    fn free_buffer(&self, bufnum: i32) -> BackendResult {
        self.client
            .free_buffer(bufnum)
            .map_err(BackendError::from)
    }

    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> BackendResult {
        self.client
            .alloc_buffer(bufnum, num_frames, num_channels)
            .map_err(BackendError::from)
    }

    fn send_raw(&self, addr: &str, args: Vec<RawArg>) -> BackendResult {
        let osc_args: Vec<rosc::OscType> = args
            .into_iter()
            .map(|a| match a {
                RawArg::Int(v) => rosc::OscType::Int(v),
                RawArg::Float(v) => rosc::OscType::Float(v),
                RawArg::Str(v) => rosc::OscType::String(v),
                RawArg::Blob(v) => rosc::OscType::Blob(v),
            })
            .collect();
        self.client
            .send_message(addr, osc_args)
            .map_err(BackendError::from)
    }
}

// ─── Test Backend ───────────────────────────────────────────────────

use std::sync::Mutex;

/// An operation recorded by `TestBackend` for assertion in tests.
#[derive(Debug, Clone, PartialEq)]
pub enum TestOp {
    CreateGroup {
        group_id: i32,
        add_action: i32,
        target: i32,
    },
    CreateSynth {
        def_name: String,
        node_id: i32,
        group_id: i32,
        params: Vec<(String, f32)>,
    },
    FreeNode(i32),
    SetParam {
        node_id: i32,
        param: String,
        value: f32,
    },
    SetParams {
        node_id: i32,
        params: Vec<(String, f32)>,
    },
    LoadBuffer {
        bufnum: i32,
        path: String,
    },
    FreeBuffer(i32),
    AllocBuffer {
        bufnum: i32,
        num_frames: i32,
        num_channels: i32,
    },
    SendRaw {
        addr: String,
        args: Vec<RawArg>,
    },
}

/// A test backend that records all operations into a vector for assertions.
/// All operations succeed by default. Uses `Mutex` for interior mutability
/// so the backend is `Send + Sync` (needed for `Arc<TestBackend>` sharing).
pub struct TestBackend {
    ops: Mutex<Vec<TestOp>>,
}

impl TestBackend {
    pub fn new() -> Self {
        Self {
            ops: Mutex::new(Vec::new()),
        }
    }

    /// Return all recorded operations.
    pub fn operations(&self) -> Vec<TestOp> {
        self.ops.lock().unwrap().clone()
    }

    /// Clear recorded operations.
    pub fn clear(&self) {
        self.ops.lock().unwrap().clear();
    }

    /// Count operations matching a predicate.
    pub fn count<F: Fn(&TestOp) -> bool>(&self, f: F) -> usize {
        self.ops.lock().unwrap().iter().filter(|op| f(op)).count()
    }

    /// Find the first operation matching a predicate.
    pub fn find<F: Fn(&TestOp) -> bool>(&self, f: F) -> Option<TestOp> {
        self.ops.lock().unwrap().iter().find(|op| f(op)).cloned()
    }

    /// Return all CreateSynth operations.
    pub fn synths_created(&self) -> Vec<TestOp> {
        self.ops
            .lock()
            .unwrap()
            .iter()
            .filter(|op| matches!(op, TestOp::CreateSynth { .. }))
            .cloned()
            .collect()
    }

    /// Return all FreeNode operations.
    pub fn nodes_freed(&self) -> Vec<i32> {
        self.ops
            .lock()
            .unwrap()
            .iter()
            .filter_map(|op| match op {
                TestOp::FreeNode(id) => Some(*id),
                _ => None,
            })
            .collect()
    }
}

impl AudioBackend for TestBackend {
    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::CreateGroup {
            group_id,
            add_action,
            target,
        });
        Ok(())
    }

    fn create_synth(
        &self,
        def_name: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::CreateSynth {
            def_name: def_name.to_string(),
            node_id,
            group_id,
            params: params.to_vec(),
        });
        Ok(())
    }

    fn free_node(&self, node_id: i32) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::FreeNode(node_id));
        Ok(())
    }

    fn set_param(&self, node_id: i32, param: &str, value: f32) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::SetParam {
            node_id,
            param: param.to_string(),
            value,
        });
        Ok(())
    }

    fn set_params(&self, node_id: i32, params: &[(&str, f32)]) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::SetParams {
            node_id,
            params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        });
        Ok(())
    }

    fn load_buffer(&self, bufnum: i32, path: &Path) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::LoadBuffer {
            bufnum,
            path: path.to_string_lossy().to_string(),
        });
        Ok(())
    }

    fn free_buffer(&self, bufnum: i32) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::FreeBuffer(bufnum));
        Ok(())
    }

    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::AllocBuffer {
            bufnum,
            num_frames,
            num_channels,
        });
        Ok(())
    }

    fn send_raw(&self, addr: &str, args: Vec<RawArg>) -> BackendResult {
        self.ops.lock().unwrap().push(TestOp::SendRaw {
            addr: addr.to_string(),
            args,
        });
        Ok(())
    }
}

impl Default for TestBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ─── NullBackend ────────────────────────────────────────────────────

/// A no-op backend that silently succeeds. Useful as a default when
/// no audio server is connected (replaces `NullOscClient` use cases).
#[allow(dead_code)]
pub struct NullBackend;

impl AudioBackend for NullBackend {
    fn create_group(&self, _: i32, _: i32, _: i32) -> BackendResult {
        Ok(())
    }

    fn create_synth(&self, _: &str, _: i32, _: i32, _: &[(String, f32)]) -> BackendResult {
        Ok(())
    }

    fn free_node(&self, _: i32) -> BackendResult {
        Ok(())
    }

    fn set_param(&self, _: i32, _: &str, _: f32) -> BackendResult {
        Ok(())
    }

    fn set_params(&self, _: i32, _: &[(&str, f32)]) -> BackendResult {
        Ok(())
    }

    fn load_buffer(&self, _: i32, _: &Path) -> BackendResult {
        Ok(())
    }

    fn free_buffer(&self, _: i32) -> BackendResult {
        Ok(())
    }

    fn alloc_buffer(&self, _: i32, _: i32, _: i32) -> BackendResult {
        Ok(())
    }

    fn send_raw(&self, _: &str, _: Vec<RawArg>) -> BackendResult {
        Ok(())
    }
}

// ─── OscClientLike adapter for TestBackend ──────────────────────────
//
// This bridges TestBackend into the existing routing code (which calls
// `self.client: Option<Box<dyn OscClientLike>>`). The adapter captures
// operations via TestBackend while satisfying the OscClientLike interface.

use std::sync::Arc;

/// Wraps a `TestBackend` (shared via `Arc`) to implement `OscClientLike`.
/// This lets the existing routing code record operations for test assertions.
pub struct TestOscAdapter {
    inner: Arc<TestBackend>,
}

impl TestOscAdapter {
    pub fn new(backend: Arc<TestBackend>) -> Self {
        Self { inner: backend }
    }
}

impl OscClientLike for TestOscAdapter {
    fn meter_peak(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn audio_in_waveform(&self, _instrument_id: u32) -> Vec<f32> {
        Vec::new()
    }

    fn send_message(&self, addr: &str, args: Vec<rosc::OscType>) -> std::io::Result<()> {
        let raw_args: Vec<RawArg> = args
            .into_iter()
            .map(|a| match a {
                rosc::OscType::Int(v) => RawArg::Int(v),
                rosc::OscType::Float(v) => RawArg::Float(v),
                rosc::OscType::String(v) => RawArg::Str(v),
                rosc::OscType::Blob(v) => RawArg::Blob(v),
                _ => RawArg::Str(format!("{:?}", a)),
            })
            .collect();
        let _ = self.inner.send_raw(addr, raw_args);
        Ok(())
    }

    fn create_group(&self, group_id: i32, add_action: i32, target: i32) -> std::io::Result<()> {
        let _ = AudioBackend::create_group(self.inner.as_ref(), group_id, add_action, target);
        Ok(())
    }

    fn create_synth(&self, synth_def: &str, node_id: i32, params: &[(String, f32)]) -> std::io::Result<()> {
        // Default group = 0
        let _ = AudioBackend::create_synth(self.inner.as_ref(), synth_def, node_id, 0, params);
        Ok(())
    }

    fn create_synth_in_group(
        &self,
        synth_def: &str,
        node_id: i32,
        group_id: i32,
        params: &[(String, f32)],
    ) -> std::io::Result<()> {
        let _ = AudioBackend::create_synth(self.inner.as_ref(), synth_def, node_id, group_id, params);
        Ok(())
    }

    fn free_node(&self, node_id: i32) -> std::io::Result<()> {
        let _ = AudioBackend::free_node(self.inner.as_ref(), node_id);
        Ok(())
    }

    fn set_param(&self, node_id: i32, param: &str, value: f32) -> std::io::Result<()> {
        let _ = AudioBackend::set_param(self.inner.as_ref(), node_id, param, value);
        Ok(())
    }

    fn set_params_bundled(
        &self,
        node_id: i32,
        params: &[(&str, f32)],
        _time: rosc::OscTime,
    ) -> std::io::Result<()> {
        let _ = AudioBackend::set_params(self.inner.as_ref(), node_id, params);
        Ok(())
    }

    fn send_bundle(
        &self,
        messages: Vec<rosc::OscMessage>,
        _time: rosc::OscTime,
    ) -> std::io::Result<()> {
        // Record each message in the bundle as a raw send
        for msg in messages {
            let raw_args: Vec<RawArg> = msg.args
                .into_iter()
                .map(|a| match a {
                    rosc::OscType::Int(v) => RawArg::Int(v),
                    rosc::OscType::Float(v) => RawArg::Float(v),
                    rosc::OscType::String(v) => RawArg::Str(v),
                    rosc::OscType::Blob(v) => RawArg::Blob(v),
                    _ => RawArg::Str(format!("{:?}", a)),
                })
                .collect();
            let _ = self.inner.send_raw(&msg.addr, raw_args);
        }
        Ok(())
    }

    fn load_buffer(&self, bufnum: i32, path: &str) -> std::io::Result<()> {
        let _ = AudioBackend::load_buffer(self.inner.as_ref(), bufnum, Path::new(path));
        Ok(())
    }

    fn alloc_buffer(&self, bufnum: i32, num_frames: i32, num_channels: i32) -> std::io::Result<()> {
        let _ = AudioBackend::alloc_buffer(self.inner.as_ref(), bufnum, num_frames, num_channels);
        Ok(())
    }

    fn free_buffer(&self, bufnum: i32) -> std::io::Result<()> {
        let _ = AudioBackend::free_buffer(self.inner.as_ref(), bufnum);
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

    fn send_unit_cmd(
        &self,
        node_id: i32,
        ugen_index: i32,
        cmd: &str,
        args: Vec<rosc::OscType>,
    ) -> std::io::Result<()> {
        let mut raw_args = vec![
            RawArg::Int(node_id),
            RawArg::Int(ugen_index),
            RawArg::Str(cmd.to_string()),
        ];
        for a in args {
            raw_args.push(match a {
                rosc::OscType::Int(v) => RawArg::Int(v),
                rosc::OscType::Float(v) => RawArg::Float(v),
                rosc::OscType::String(v) => RawArg::Str(v),
                rosc::OscType::Blob(v) => RawArg::Blob(v),
                _ => RawArg::Str(format!("{:?}", a)),
            });
        }
        let _ = self.inner.send_raw("/u_cmd", raw_args);
        Ok(())
    }
}
