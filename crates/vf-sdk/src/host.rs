use crate::error::SdkError;
use std::ffi::CStr;
use vf_abi::{VF_LOG_DEBUG, VF_LOG_ERROR, VF_LOG_INFO, VF_LOG_WARN, VfHostApi};

/// Host callbacks available to a node instance.
#[derive(Clone, Copy)]
pub struct Host {
    api: *const VfHostApi,
    node_handle: u64,
}

unsafe impl Send for Host {}
unsafe impl Sync for Host {}

impl Host {
    /// # Safety
    /// `api` must remain valid for the lifetime of the plugin process.
    pub unsafe fn from_raw(api: *const VfHostApi, node_handle: u64) -> Self {
        Self { api, node_handle }
    }

    pub fn now_us(self) -> u64 {
        unsafe {
            if self.api.is_null() {
                return 0;
            }
            let api = &*self.api;
            (api.now_us)(api.user_data)
        }
    }

    pub fn wake(self) {
        unsafe {
            if self.api.is_null() {
                return;
            }
            let api = &*self.api;
            (api.wake)(api.user_data, self.node_handle);
        }
    }

    pub fn log(self, level: u32, msg: &str) {
        unsafe {
            if self.api.is_null() {
                return;
            }
            let api = &*self.api;
            let mut buf = Vec::with_capacity(msg.len() + 1);
            buf.extend_from_slice(msg.as_bytes());
            buf.push(0);
            (api.log)(api.user_data, level, self.node_handle, buf.as_ptr().cast());
        }
    }

    pub fn error(self, msg: &str) {
        self.log(VF_LOG_ERROR, msg);
    }
    pub fn warn(self, msg: &str) {
        self.log(VF_LOG_WARN, msg);
    }
    pub fn info(self, msg: &str) {
        self.log(VF_LOG_INFO, msg);
    }
    pub fn debug(self, msg: &str) {
        self.log(VF_LOG_DEBUG, msg);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessCtx {
    pub tick: u64,
    pub dt_us: u64,
    pub now_us: u64,
}

impl ProcessCtx {
    pub fn dt_secs(self) -> f32 {
        (self.dt_us as f32 / 1_000_000.0).max(1e-4)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatusKind {
    Ok,
    Warn,
    Error,
    Idle,
}

#[derive(Clone, Debug)]
pub struct NodeStatus {
    pub kind: NodeStatusKind,
    pub message: String,
}

impl NodeStatus {
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            kind: NodeStatusKind::Ok,
            message: msg.into(),
        }
    }
    pub fn warn(msg: impl Into<String>) -> Self {
        Self {
            kind: NodeStatusKind::Warn,
            message: msg.into(),
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            kind: NodeStatusKind::Error,
            message: msg.into(),
        }
    }
    pub fn idle(msg: impl Into<String>) -> Self {
        Self {
            kind: NodeStatusKind::Idle,
            message: msg.into(),
        }
    }
}

/// # Safety
/// `p` must be null or a valid, NUL-terminated C string that outlives the returned borrow.
pub unsafe fn cstr_opt<'a>(p: *const std::ffi::c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(p).to_str().ok() }
    }
}

/// # Safety
/// `p` must be null or a valid, NUL-terminated C string.
pub unsafe fn parse_config_json(p: *const std::ffi::c_char) -> serde_json::Value {
    match unsafe { cstr_opt(p) } {
        None | Some("") => serde_json::json!({}),
        Some(s) => serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({})),
    }
}

/// # Safety
/// `p` must be null or a valid, NUL-terminated C string.
pub unsafe fn json_from_ptr(p: *const std::ffi::c_char) -> Result<serde_json::Value, SdkError> {
    match unsafe { cstr_opt(p) } {
        None | Some("") => Ok(serde_json::Value::Null),
        Some(s) => serde_json::from_str(s).map_err(|e| SdkError::Param(e.to_string())),
    }
}
