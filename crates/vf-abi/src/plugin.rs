//! Plugin descriptor, node vtable, and host callback table.

use crate::value::VfValueTag;
use core::ffi::{c_char, c_void};

/// Bump only on breaking layout changes. Hosts refuse plugins with a different version.
pub const VF_ABI_VERSION: u32 = 2;

pub const VF_OK: i32 = 0;
pub const VF_ERR: i32 = -1;
pub const VF_NO_NEW_DATA: i32 = 1;

pub const VF_NODE_IS_SOURCE: u32 = 1 << 0;

pub const VF_LOG_ERROR: u32 = 0;
pub const VF_LOG_WARN: u32 = 1;
pub const VF_LOG_INFO: u32 = 2;
pub const VF_LOG_DEBUG: u32 = 3;

pub const VF_STATUS_OK: u32 = 0;
pub const VF_STATUS_WARN: u32 = 1;
pub const VF_STATUS_ERROR: u32 = 2;
pub const VF_STATUS_IDLE: u32 = 3;

pub type VfStatus = i32;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VfCategory {
    Input = 0,
    Process = 1,
    Output = 2,
    Utility = 3,
}

impl VfCategory {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Process => "process",
            Self::Output => "output",
            Self::Utility => "utility",
        }
    }
}

pub const VF_PLUGIN_ENTRY: &[u8] = b"vf_plugin_entry\0";

pub type VfPluginEntry = unsafe extern "C" fn(host_abi: u32) -> *const VfPluginDescriptor;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfPluginDescriptor {
    pub abi_version: u32,
    pub _pad: u32,
    pub id: *const c_char,
    pub name: *const c_char,
    pub version: *const c_char,
    pub node_count: u32,
    pub _pad2: u32,
    pub nodes: *const VfNodeDescriptor,
    pub description: *const c_char,
    pub author: *const c_char,
    pub license: *const c_char,
    pub homepage: *const c_char,
    pub repository: *const c_char,
    pub issues: *const c_char,
    /// Comma-separated keywords; empty string if none.
    pub keywords: *const c_char,
}

unsafe impl Send for VfPluginDescriptor {}
unsafe impl Sync for VfPluginDescriptor {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfPortDesc {
    pub name: *const c_char,
    pub ty: VfValueTag,
    pub _pad: u32,
    pub schema: *const c_char,
    pub capacity: u32,
    pub _pad2: u32,
}

unsafe impl Send for VfPortDesc {}
unsafe impl Sync for VfPortDesc {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfNodeDescriptor {
    pub type_id: *const c_char,
    pub display_name: *const c_char,
    pub category: VfCategory,
    pub flags: u32,
    pub inputs: *const VfPortDesc,
    pub input_count: u32,
    pub _pad: u32,
    pub outputs: *const VfPortDesc,
    pub output_count: u32,
    pub _pad2: u32,
    pub params_schema_json: *const c_char,
    pub create: unsafe extern "C" fn(
        host: *const VfHostApi,
        node_handle: u64,
        config_json: *const c_char,
    ) -> *mut c_void,
    pub vtable: *const VfNodeVTable,
}

unsafe impl Send for VfNodeDescriptor {}
unsafe impl Sync for VfNodeDescriptor {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfProcessCtx {
    pub tick: u64,
    pub dt_us: u64,
    pub now_us: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfNodeStatus {
    pub level: u32,
    pub message: [u8; 256],
}

impl Default for VfNodeStatus {
    fn default() -> Self {
        Self {
            level: VF_STATUS_IDLE,
            message: [0; 256],
        }
    }
}

impl VfNodeStatus {
    pub fn set_message(&mut self, text: &str) {
        self.message = [0; 256];
        let bytes = text.as_bytes();
        let n = bytes.len().min(255);
        self.message[..n].copy_from_slice(&bytes[..n]);
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfNodeVTable {
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub start: unsafe extern "C" fn(*mut c_void) -> VfStatus,
    pub stop: unsafe extern "C" fn(*mut c_void),
    pub process: unsafe extern "C" fn(
        *mut c_void,
        ctx: *const VfProcessCtx,
        inputs: *const crate::value::VfValue,
        n_in: u32,
        outputs: *mut crate::value::VfValue,
        n_out: u32,
    ) -> VfStatus,
    pub set_param: unsafe extern "C" fn(
        *mut c_void,
        key: *const c_char,
        value_json: *const c_char,
    ) -> VfStatus,
    pub get_state: unsafe extern "C" fn(*mut c_void, buf: *mut u8, cap: usize) -> usize,
    pub set_state: unsafe extern "C" fn(*mut c_void, json: *const c_char) -> VfStatus,
    pub status: unsafe extern "C" fn(*mut c_void, out: *mut VfNodeStatus),
}

unsafe impl Send for VfNodeVTable {}
unsafe impl Sync for VfNodeVTable {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VfHostApi {
    pub user_data: *mut c_void,
    pub log: unsafe extern "C" fn(
        user_data: *mut c_void,
        level: u32,
        node_handle: u64,
        msg: *const c_char,
    ),
    pub wake: unsafe extern "C" fn(user_data: *mut c_void, node_handle: u64),
    pub now_us: unsafe extern "C" fn(user_data: *mut c_void) -> u64,
}

unsafe impl Send for VfHostApi {}
unsafe impl Sync for VfHostApi {}
