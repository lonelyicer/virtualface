use crate::registry::{NodeRegistry, NodeType, PortType};
use crate::vars::VarType;
use vf_abi::{VfNodeStatus, VfNodeVTable, VfProcessCtx, VfStatus, VfValue, VfValueTag};
use vf_sdk::{Category, ParamDef};

struct BuiltinSlot;

unsafe extern "C" fn builtin_create(
    _: *const vf_abi::VfHostApi,
    _: u64,
    _: *const std::ffi::c_char,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(BuiltinSlot)) as *mut std::ffi::c_void
}

unsafe extern "C" fn builtin_destroy(ptr: *mut std::ffi::c_void) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr as *mut BuiltinSlot) });
    }
}

unsafe extern "C" fn builtin_ok(_: *mut std::ffi::c_void) -> VfStatus {
    vf_abi::VF_OK
}

unsafe extern "C" fn builtin_stop(_: *mut std::ffi::c_void) {}

unsafe extern "C" fn builtin_process(
    _: *mut std::ffi::c_void,
    _: *const VfProcessCtx,
    _: *const VfValue,
    _: u32,
    _: *mut VfValue,
    _: u32,
) -> VfStatus {
    vf_abi::VF_OK
}

unsafe extern "C" fn builtin_param(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_char,
    _: *const std::ffi::c_char,
) -> VfStatus {
    vf_abi::VF_OK
}

unsafe extern "C" fn builtin_get_state(_: *mut std::ffi::c_void, _: *mut u8, _: usize) -> usize {
    0
}

unsafe extern "C" fn builtin_set_state(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_char,
) -> VfStatus {
    vf_abi::VF_OK
}

unsafe extern "C" fn builtin_status(_: *mut std::ffi::c_void, out: *mut VfNodeStatus) {
    if !out.is_null() {
        unsafe { *out = VfNodeStatus::default() };
    }
}

fn vtable() -> VfNodeVTable {
    VfNodeVTable {
        destroy: builtin_destroy,
        start: builtin_ok,
        stop: builtin_stop,
        process: builtin_process,
        set_param: builtin_param,
        get_state: builtin_get_state,
        set_state: builtin_set_state,
        status: builtin_status,
    }
}

fn port_schema(name: &str, tag: VfValueTag, schema: &str, capacity: u32) -> PortType {
    PortType {
        name: name.into(),
        tag: tag as u32,
        schema: schema.into(),
        capacity,
    }
}

fn builtin_type(
    type_id: &str,
    display: &str,
    inputs: Vec<PortType>,
    outputs: Vec<PortType>,
) -> NodeType {
    NodeType {
        plugin_id: "virtualface".into(),
        plugin_name: "VirtualFace".into(),
        type_id: type_id.into(),
        display_name: display.into(),
        category: Category::Utility,
        flags: 0,
        inputs,
        outputs,
        params: vec![ParamDef::string("name", "Name", "")],
        create: builtin_create,
        vtable: vtable(),
    }
}

pub fn register_builtins(reg: &mut NodeRegistry) {
    for ty in VarType::ALL {
        let tag = ty.tag();
        let label = ty.label();
        let (schema, cap) = if matches!(ty, VarType::String) {
            ("utf8", 256)
        } else {
            ("", 0)
        };
        let value = port_schema("Value", tag, schema, cap);
        reg.register(builtin_type(
            ty.get_type_id(),
            &format!("Get {label}"),
            vec![],
            vec![value.clone()],
        ));
        reg.register(builtin_type(
            ty.set_type_id(),
            &format!("Set {label}"),
            vec![value.clone()],
            vec![value],
        ));
    }
}

pub fn is_builtin(type_id: &str) -> bool {
    VarType::from_type_id(type_id).is_some()
}
