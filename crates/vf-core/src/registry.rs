use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use vf_abi::{VfNodeDescriptor, VfNodeVTable, VfPortDesc, VfValueTag};
use vf_sdk::{Category, ParamDef, ParamKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortType {
    pub name: String,
    pub tag: u32,
    pub schema: String,
    pub capacity: u32,
}

impl PortType {
    pub fn value_tag(&self) -> VfValueTag {
        match self.tag {
            1 => VfValueTag::Float,
            2 => VfValueTag::Int,
            3 => VfValueTag::Bool,
            4 => VfValueTag::Vec2,
            5 => VfValueTag::Vec3,
            6 => VfValueTag::UnifiedFrame,
            8 => VfValueTag::Bytes,
            _ => VfValueTag::Empty,
        }
    }

    pub fn from_c(p: &VfPortDesc) -> Self {
        Self {
            name: unsafe { cstr(p.name) },
            tag: p.ty as u32,
            schema: unsafe { cstr(p.schema) },
            capacity: p.capacity,
        }
    }
}

#[derive(Clone)]
pub struct NodeType {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub category: Category,
    pub inputs: Vec<PortType>,
    pub outputs: Vec<PortType>,
    pub params: Vec<ParamDef>,
    pub create: unsafe extern "C" fn(
        host: *const vf_abi::VfHostApi,
        node_handle: u64,
        config_json: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void,
    pub vtable: VfNodeVTable,
}

impl NodeType {
    pub fn pin_params(&self) -> impl Iterator<Item = &ParamDef> {
        self.params.iter().filter(|p| p.kind != ParamKind::Button)
    }

    pub fn param_kind_tag(kind: ParamKind) -> Option<VfValueTag> {
        match kind {
            ParamKind::Float => Some(VfValueTag::Float),
            ParamKind::Int => Some(VfValueTag::Int),
            ParamKind::Bool => Some(VfValueTag::Bool),
            ParamKind::String => Some(VfValueTag::Bytes),
            _ => None,
        }
    }

    pub fn input_tag(&self, port: u32) -> Option<VfValueTag> {
        if let Some(p) = self.inputs.get(port as usize) {
            return Some(p.value_tag());
        }
        let i = (port as usize).checked_sub(self.inputs.len())?;
        let p = self.pin_params().nth(i)?;
        Self::param_kind_tag(p.kind)
    }

    pub fn param_key_for_port(&self, port: u32) -> Option<&str> {
        let i = (port as usize).checked_sub(self.inputs.len())?;
        self.pin_params().nth(i).map(|p| p.key.as_str())
    }

    pub fn input_is_wirable(&self, port: u32) -> bool {
        if (port as usize) < self.inputs.len() {
            return true;
        }
        self.input_tag(port).is_some()
    }

    /// # Safety
    /// `d` and every pointer it contains must be valid for the duration of this call.
    pub unsafe fn from_c(plugin_id: &str, plugin_name: &str, d: &VfNodeDescriptor) -> Result<Self> {
        if d.vtable.is_null() {
            return Err(CoreError::Load("null vtable".into()));
        }
        let inputs = unsafe { ports(d.inputs, d.input_count) };
        let outputs = unsafe { ports(d.outputs, d.output_count) };
        let schema_json = unsafe { cstr(d.params_schema_json) };
        let params: Vec<ParamDef> = if schema_json.is_empty() {
            vec![]
        } else {
            serde_json::from_str(&schema_json).unwrap_or_default()
        };
        Ok(Self {
            plugin_id: plugin_id.into(),
            plugin_name: plugin_name.into(),
            type_id: unsafe { cstr(d.type_id) },
            display_name: unsafe { cstr(d.display_name) },
            category: Category::from(d.category),
            inputs,
            outputs,
            params,
            create: d.create,
            vtable: unsafe { *d.vtable },
        })
    }
}

unsafe fn ports(ptr: *const VfPortDesc, n: u32) -> Vec<PortType> {
    if ptr.is_null() {
        return vec![];
    }
    unsafe {
        std::slice::from_raw_parts(ptr, n as usize)
            .iter()
            .map(PortType::from_c)
            .collect()
    }
}

unsafe fn cstr(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() }
}

#[derive(Clone, Default)]
pub struct NodeRegistry {
    types: Vec<NodeType>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, ty: NodeType) {
        if let Some(existing) = self.types.iter_mut().find(|t| t.type_id == ty.type_id) {
            *existing = ty;
        } else {
            self.types.push(ty);
        }
    }

    pub fn get(&self, type_id: &str) -> Option<&NodeType> {
        self.types.iter().find(|t| t.type_id == type_id)
    }

    pub fn all(&self) -> &[NodeType] {
        &self.types
    }
}
