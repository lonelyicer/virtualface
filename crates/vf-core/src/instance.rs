use crate::error::{CoreError, Result};
use crate::registry::NodeType;
use std::ffi::{CString, c_void};
use vf_abi::{
    VF_ERR, VF_OK, VfHostApi, VfNodeStatus, VfProcessCtx, VfStatus, VfUnifiedFrame, VfValue,
    VfValueTag,
};

pub struct NodeInstance {
    ptr: *mut c_void,
    ty: NodeType,
    pub disabled: bool,
}

unsafe impl Send for NodeInstance {}

impl NodeInstance {
    pub fn create(
        ty: &NodeType,
        host: *const VfHostApi,
        handle: u64,
        config: &serde_json::Value,
    ) -> Result<Self> {
        let json = serde_json::to_string(config).unwrap_or_else(|_| "{}".into());
        let c = CString::new(json).unwrap_or_else(|_| CString::new("{}").unwrap());
        let ptr = unsafe { (ty.create)(host, handle, c.as_ptr()) };
        if ptr.is_null() {
            return Err(CoreError::Engine(format!(
                "node '{}' create returned null",
                ty.type_id
            )));
        }
        Ok(Self {
            ptr,
            ty: ty.clone(),
            disabled: false,
        })
    }

    pub fn type_id(&self) -> &str {
        &self.ty.type_id
    }

    pub fn node_type(&self) -> &NodeType {
        &self.ty
    }

    pub fn start(&mut self) -> Result<()> {
        let st = unsafe { (self.ty.vtable.start)(self.ptr) };
        if st == VF_ERR {
            Err(CoreError::Engine(format!(
                "{} start failed",
                self.ty.type_id
            )))
        } else {
            Ok(())
        }
    }

    pub fn stop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe { (self.ty.vtable.stop)(self.ptr) };
    }

    pub fn process(
        &mut self,
        ctx: &VfProcessCtx,
        inputs: &[VfValue],
        outputs: &mut [VfValue],
    ) -> VfStatus {
        if self.disabled {
            return VF_ERR;
        }
        // Do not catch_unwind around the FFI call: panics in the plugin use a
        // different panic runtime. The plugin trampoline already converts panics
        // to VF_ERR.
        unsafe {
            (self.ty.vtable.process)(
                self.ptr,
                ctx,
                inputs.as_ptr(),
                inputs.len() as u32,
                outputs.as_mut_ptr(),
                outputs.len() as u32,
            )
        }
    }

    pub fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        let k =
            CString::new(key).map_err(|_| CoreError::Engine("param key contains NUL".into()))?;
        let json = serde_json::to_string(value)?;
        let v =
            CString::new(json).map_err(|_| CoreError::Engine("param value contains NUL".into()))?;
        let st = unsafe { (self.ty.vtable.set_param)(self.ptr, k.as_ptr(), v.as_ptr()) };
        if st == VF_OK {
            Ok(())
        } else {
            Err(CoreError::Engine(format!("set_param {key} failed")))
        }
    }

    pub fn get_state(&self) -> Option<serde_json::Value> {
        let needed = unsafe { (self.ty.vtable.get_state)(self.ptr, std::ptr::null_mut(), 0) };
        if needed == 0 {
            return None;
        }
        let mut buf = vec![0u8; needed];
        let n = unsafe { (self.ty.vtable.get_state)(self.ptr, buf.as_mut_ptr(), buf.len()) };
        buf.truncate(n.min(buf.len()));
        serde_json::from_slice(&buf).ok()
    }

    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<()> {
        let json = serde_json::to_string(state)?;
        let c = CString::new(json).map_err(|_| CoreError::Engine("state contains NUL".into()))?;
        let st = unsafe { (self.ty.vtable.set_state)(self.ptr, c.as_ptr()) };
        if st == VF_OK {
            Ok(())
        } else {
            Err(CoreError::Engine("set_state failed".into()))
        }
    }

    pub fn status(&self) -> VfNodeStatus {
        let mut out = VfNodeStatus::default();
        unsafe { (self.ty.vtable.status)(self.ptr, &mut out) };
        out
    }
}

impl Drop for NodeInstance {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (self.ty.vtable.destroy)(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}

pub enum PortBuffer {
    Scalar,
    Unified(Box<VfUnifiedFrame>),
    Bytes { data: Vec<u8>, tag: CString },
}

impl PortBuffer {
    pub fn from_port(tag: VfValueTag, schema: &str, capacity: u32) -> Self {
        match tag {
            VfValueTag::UnifiedFrame => PortBuffer::Unified(Box::new(VfUnifiedFrame::default())),
            VfValueTag::Bytes => {
                let cap = capacity.max(1) as usize;
                PortBuffer::Bytes {
                    data: vec![0; cap],
                    tag: CString::new(schema).unwrap_or_default(),
                }
            }
            _ => PortBuffer::Scalar,
        }
    }

    pub fn as_value(&mut self, tag: VfValueTag) -> VfValue {
        match self {
            PortBuffer::Unified(f) => VfValue::unified(f.as_mut()),
            PortBuffer::Bytes { data, tag: t } => VfValue {
                tag: VfValueTag::Bytes,
                _pad: 0,
                payload: vf_abi::VfPayload {
                    bytes: vf_abi::VfBytes {
                        tag: t.as_ptr(),
                        ptr: data.as_mut_ptr(),
                        len: data.len() as u32,
                        cap: data.len() as u32,
                    },
                },
            },
            PortBuffer::Scalar => VfValue {
                tag,
                _pad: 0,
                payload: vf_abi::VfPayload::default(),
            },
        }
    }
}

pub fn snapshot_value(v: &VfValue) -> SnapshotValue {
    match v.tag {
        VfValueTag::Float => SnapshotValue::Float(unsafe { v.payload.float }),
        VfValueTag::Int => SnapshotValue::Int(unsafe { v.payload.int }),
        VfValueTag::Bool => SnapshotValue::Bool(unsafe { v.payload.boolean } != 0),
        VfValueTag::Vec2 => SnapshotValue::Vec2(unsafe { v.payload.vec2 }),
        VfValueTag::Vec3 => SnapshotValue::Vec3(unsafe { v.payload.vec3 }),
        VfValueTag::UnifiedFrame => {
            let f = unsafe { v.as_unified() };
            SnapshotValue::Unified(f.cloned().unwrap_or_default())
        }
        VfValueTag::Bytes => {
            let b = unsafe { v.payload.bytes };
            if b.ptr.is_null() || b.len == 0 {
                SnapshotValue::Text(String::new())
            } else {
                let sl = unsafe { std::slice::from_raw_parts(b.ptr, b.len as usize) };
                SnapshotValue::Text(String::from_utf8_lossy(sl).into_owned())
            }
        }
        _ => SnapshotValue::Empty,
    }
}

#[derive(Clone, Debug)]
pub enum SnapshotValue {
    Empty,
    Float(f32),
    Int(i64),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Unified(VfUnifiedFrame),
    Text(String),
}

impl SnapshotValue {
    pub fn payload_bytes(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Float(_) | Self::Bool(_) => 4,
            Self::Int(_) => 8,
            Self::Vec2(_) => 8,
            Self::Vec3(_) => 12,
            Self::Unified(_) => std::mem::size_of::<VfUnifiedFrame>(),
            Self::Text(s) => s.len(),
        }
    }

    pub fn preview(&self) -> String {
        match self {
            Self::Empty => "—".into(),
            Self::Float(v) => format!("{v:.3}"),
            Self::Int(v) => v.to_string(),
            Self::Bool(v) => v.to_string(),
            Self::Vec2(v) => format!("{:.2},{:.2}", v[0], v[1]),
            Self::Vec3(v) => format!("{:.2},{:.2},{:.2}", v[0], v[1], v[2]),
            Self::Unified(f) => {
                let jaw = f.shapes[vf_abi::UnifiedExpression::JawOpen.index()];
                format!(
                    "jaw {jaw:.2}  eye {:.2}/{:.2}",
                    f.eye.left.openness, f.eye.right.openness
                )
            }
            Self::Text(s) => {
                if s.chars().count() > 24 {
                    format!("{}…", s.chars().take(24).collect::<String>())
                } else {
                    s.clone()
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeSnap {
    pub status_level: u32,
    pub status_text: String,
    pub outputs: Vec<SnapshotValue>,
    pub disabled: bool,
    pub state: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub tick: u64,
    /// Duration of the last engine tick, in microseconds.
    pub dt_us: u64,
    pub running: bool,
    pub drops: u64,
    pub last_tick_us: u64,
    pub nodes: std::collections::HashMap<u64, NodeSnap>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            tick: 0,
            dt_us: 0,
            running: false,
            drops: 0,
            last_tick_us: 0,
            nodes: std::collections::HashMap::new(),
        }
    }
}
