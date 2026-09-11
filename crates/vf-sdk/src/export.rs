/// Build C ABI trampolines and `vf_plugin_entry`.
///
/// Catalog fields may appear in any order. Optional: `description`, `author`,
/// `license`, `homepage`, `repository`, `issues`, `keywords` (comma-separated).
///
/// ```ignore
/// vf_sdk::export_plugin! {
///     id: "vf.pico",
///     name: "PICO Input",
///     version: "0.1.0",
///     description: "PICO Connect UDP face tracking",
///     repository: "https://github.com/lonelyicer/virtualface",
///     nodes: [PicoUdpSource]
/// }
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($($input:tt)*) => {
        $crate::__export_plugin_parse! {
            @id() @name() @version()
            @description() @author() @license()
            @homepage() @repository() @issues() @keywords()
            @nodes()
            $($input)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __export_plugin_parse {
    (
        @id($id:literal) @name($name:literal) @version($version:literal)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),+)
    ) => {
        $crate::__export_plugin_emit! {
            $id $name $version
            [$($description)?] [$($author)?] [$($license)?]
            [$($homepage)?] [$($repository)?] [$($issues)?] [$($keywords)?]
            [$($node),+]
        }
    };

    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        id: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($new) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        name: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($new) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        version: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($new)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        description: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($new) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        author: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($new)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        license: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($new) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        homepage: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($new)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        repository: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($new) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        issues: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($new)
            @keywords($($keywords)?)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        keywords: $new:literal, $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($new)
            @nodes($($node),*)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        nodes: [$($new:path),+ $(,)?], $($rest:tt)*
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($new),+)
            $($rest)*
        }
    };
    (
        @id($($id:literal)?) @name($($name:literal)?) @version($($version:literal)?)
        @description($($description:literal)?) @author($($author:literal)?)
        @license($($license:literal)?) @homepage($($homepage:literal)?)
        @repository($($repository:literal)?) @issues($($issues:literal)?)
        @keywords($($keywords:literal)?)
        @nodes($($node:path),*)
        nodes: [$($new:path),+ $(,)?]
    ) => {
        $crate::__export_plugin_parse! {
            @id($($id)?) @name($($name)?) @version($($version)?)
            @description($($description)?) @author($($author)?)
            @license($($license)?) @homepage($($homepage)?)
            @repository($($repository)?) @issues($($issues)?)
            @keywords($($keywords)?)
            @nodes($($new),+)
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __opt_str {
    () => {
        ""
    };
    ($v:literal) => {
        $v
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __export_plugin_emit {
    (
        $id:literal $name:literal $version:literal
        [$($description:literal)?] [$($author:literal)?] [$($license:literal)?]
        [$($homepage:literal)?] [$($repository:literal)?] [$($issues:literal)?]
        [$($keywords:literal)?]
        [$($node:path),+]
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn vf_plugin_entry(host_abi: u32) -> *const ::vf_abi::VfPluginDescriptor {
            if host_abi != ::vf_abi::VF_ABI_VERSION {
                return core::ptr::null();
            }
            static DESC: ::std::sync::OnceLock<::vf_abi::VfPluginDescriptor> =
                ::std::sync::OnceLock::new();
            DESC.get_or_init(|| {
                let mut nodes: Vec<::vf_abi::VfNodeDescriptor> = Vec::new();
                $(
                    nodes.push($crate::export::__abi::node_descriptor::<$node>());
                )+
                let nodes = nodes.leak();
                ::vf_abi::VfPluginDescriptor {
                    abi_version: ::vf_abi::VF_ABI_VERSION,
                    _pad: 0,
                    id: concat!($id, "\0").as_ptr().cast(),
                    name: concat!($name, "\0").as_ptr().cast(),
                    version: concat!($version, "\0").as_ptr().cast(),
                    node_count: nodes.len() as u32,
                    _pad2: 0,
                    nodes: nodes.as_ptr(),
                    description: concat!($crate::__opt_str!($($description)?), "\0").as_ptr().cast(),
                    author: concat!($crate::__opt_str!($($author)?), "\0").as_ptr().cast(),
                    license: concat!($crate::__opt_str!($($license)?), "\0").as_ptr().cast(),
                    homepage: concat!($crate::__opt_str!($($homepage)?), "\0").as_ptr().cast(),
                    repository: concat!($crate::__opt_str!($($repository)?), "\0").as_ptr().cast(),
                    issues: concat!($crate::__opt_str!($($issues)?), "\0").as_ptr().cast(),
                    keywords: concat!($crate::__opt_str!($($keywords)?), "\0").as_ptr().cast(),
                }
            }) as *const _
        }
    };
}

/// Implementation details used by [`export_plugin!`]. Not a public API.
pub mod __abi {
    use crate::desc::PortDesc;
    use crate::error::Result;
    use crate::host::{Host, NodeStatusKind, ProcessCtx, json_from_ptr, parse_config_json};
    use crate::node::Node;
    use crate::ports::PortIo;
    use std::ffi::{c_char, c_void};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use vf_abi::{
        VF_ERR, VF_OK, VF_STATUS_ERROR, VF_STATUS_IDLE, VF_STATUS_OK, VF_STATUS_WARN, VfHostApi,
        VfNodeDescriptor, VfNodeStatus, VfNodeVTable, VfPortDesc, VfProcessCtx, VfStatus, VfValue,
    };

    pub fn node_descriptor<T: Node>() -> VfNodeDescriptor {
        let meta = T::descriptor();
        let inputs = leak_ports(&meta.inputs);
        let outputs = leak_ports(&meta.outputs);
        let schema =
            leak_cstr(&serde_json::to_string(&meta.params).unwrap_or_else(|_| "[]".into()));
        VfNodeDescriptor {
            type_id: leak_cstr(meta.type_id),
            display_name: leak_cstr(meta.display_name),
            category: meta.category.into(),
            flags: meta.flags,
            inputs,
            input_count: meta.inputs.len() as u32,
            _pad: 0,
            outputs,
            output_count: meta.outputs.len() as u32,
            _pad2: 0,
            params_schema_json: schema,
            create: create::<T>,
            vtable: leak_vtable::<T>(),
        }
    }

    fn leak_ports(ports: &[PortDesc]) -> *const VfPortDesc {
        let v: Vec<VfPortDesc> = ports
            .iter()
            .map(|p| VfPortDesc {
                name: leak_cstr(p.name),
                ty: p.ty,
                _pad: 0,
                schema: leak_cstr(p.schema),
                capacity: p.capacity,
                _pad2: 0,
            })
            .collect();
        v.leak().as_ptr()
    }

    fn leak_cstr(s: &str) -> *const c_char {
        let mut v = s.as_bytes().to_vec();
        if !v.ends_with(&[0]) {
            v.push(0);
        }
        Box::leak(v.into_boxed_slice()).as_ptr().cast()
    }

    fn leak_vtable<T: Node>() -> *const VfNodeVTable {
        Box::leak(Box::new(VfNodeVTable {
            destroy: destroy::<T>,
            start: start::<T>,
            stop: stop::<T>,
            process: process::<T>,
            set_param: set_param::<T>,
            get_state: get_state::<T>,
            set_state: set_state::<T>,
            status: status::<T>,
        }))
    }

    unsafe extern "C" fn create<T: Node>(
        host: *const VfHostApi,
        node_handle: u64,
        config_json: *const c_char,
    ) -> *mut c_void {
        let host = unsafe { Host::from_raw(host, node_handle) };
        let config = parse_config_json(config_json);
        match catch_unwind(AssertUnwindSafe(|| T::create(host, &config))) {
            Ok(Ok(node)) => Box::into_raw(Box::new(node)).cast(),
            Ok(Err(e)) => {
                host.error(&format!("create failed: {e}"));
                std::ptr::null_mut()
            }
            Err(_) => {
                host.error("create panicked");
                std::ptr::null_mut()
            }
        }
    }

    unsafe extern "C" fn destroy<T: Node>(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }
        drop(unsafe { Box::from_raw(ptr.cast::<T>()) });
    }

    unsafe extern "C" fn start<T: Node>(ptr: *mut c_void) -> VfStatus {
        with_node::<T, _>(ptr, |n| n.start())
    }

    unsafe extern "C" fn stop<T: Node>(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }
        let node = unsafe { &mut *ptr.cast::<T>() };
        let _ = catch_unwind(AssertUnwindSafe(|| node.stop()));
    }

    unsafe extern "C" fn process<T: Node>(
        ptr: *mut c_void,
        ctx: *const VfProcessCtx,
        inputs: *const VfValue,
        n_in: u32,
        outputs: *mut VfValue,
        n_out: u32,
    ) -> VfStatus {
        if ptr.is_null() || ctx.is_null() {
            return VF_ERR;
        }
        let node = unsafe { &mut *ptr.cast::<T>() };
        let ctx = unsafe { &*ctx };
        let inputs = if inputs.is_null() {
            &[][..]
        } else {
            unsafe { core::slice::from_raw_parts(inputs, n_in as usize) }
        };
        let outputs = if outputs.is_null() {
            &mut [][..]
        } else {
            unsafe { core::slice::from_raw_parts_mut(outputs, n_out as usize) }
        };
        let mut io = PortIo { inputs, outputs };
        let pctx = ProcessCtx {
            tick: ctx.tick,
            dt_us: ctx.dt_us,
            now_us: ctx.now_us,
        };
        match catch_unwind(AssertUnwindSafe(|| node.process(&pctx, &mut io))) {
            Ok(Ok(())) => VF_OK,
            _ => VF_ERR,
        }
    }

    unsafe extern "C" fn set_param<T: Node>(
        ptr: *mut c_void,
        key: *const c_char,
        value_json: *const c_char,
    ) -> VfStatus {
        if ptr.is_null() {
            return VF_ERR;
        }
        let node = unsafe { &mut *ptr.cast::<T>() };
        let key = unsafe { crate::host::cstr_opt(key) }.unwrap_or("");
        let value = json_from_ptr(value_json).unwrap_or(serde_json::Value::Null);
        match catch_unwind(AssertUnwindSafe(|| node.set_param(key, &value))) {
            Ok(Ok(())) => VF_OK,
            _ => VF_ERR,
        }
    }

    unsafe extern "C" fn get_state<T: Node>(ptr: *mut c_void, buf: *mut u8, cap: usize) -> usize {
        if ptr.is_null() {
            return 0;
        }
        let node = unsafe { &*ptr.cast::<T>() };
        let json = match catch_unwind(AssertUnwindSafe(|| node.state())) {
            Ok(Some(v)) => serde_json::to_string(&v).unwrap_or_default(),
            _ => return 0,
        };
        let bytes = json.as_bytes();
        if buf.is_null() || cap == 0 {
            return bytes.len();
        }
        let n = bytes.len().min(cap);
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, n);
        }
        bytes.len()
    }

    unsafe extern "C" fn set_state<T: Node>(ptr: *mut c_void, json: *const c_char) -> VfStatus {
        if ptr.is_null() {
            return VF_ERR;
        }
        let node = unsafe { &mut *ptr.cast::<T>() };
        let value = json_from_ptr(json).unwrap_or(serde_json::Value::Null);
        match catch_unwind(AssertUnwindSafe(|| node.set_state(&value))) {
            Ok(Ok(())) => VF_OK,
            _ => VF_ERR,
        }
    }

    unsafe extern "C" fn status<T: Node>(ptr: *mut c_void, out: *mut VfNodeStatus) {
        if ptr.is_null() || out.is_null() {
            return;
        }
        let node = unsafe { &*ptr.cast::<T>() };
        let st = catch_unwind(AssertUnwindSafe(|| node.status()))
            .unwrap_or_else(|_| crate::host::NodeStatus::error("status panicked"));
        let out = unsafe { &mut *out };
        out.level = match st.kind {
            NodeStatusKind::Ok => VF_STATUS_OK,
            NodeStatusKind::Warn => VF_STATUS_WARN,
            NodeStatusKind::Error => VF_STATUS_ERROR,
            NodeStatusKind::Idle => VF_STATUS_IDLE,
        };
        out.set_message(&st.message);
    }

    fn with_node<T: Node, F>(ptr: *mut c_void, f: F) -> VfStatus
    where
        F: FnOnce(&mut T) -> Result<()>,
    {
        if ptr.is_null() {
            return VF_ERR;
        }
        let node = unsafe { &mut *ptr.cast::<T>() };
        match catch_unwind(AssertUnwindSafe(|| f(node))) {
            Ok(Ok(())) => VF_OK,
            _ => VF_ERR,
        }
    }
}
