use crate::graph::{ExecPlan, Graph, NodeId};
use crate::instance::{NodeInstance, PortBuffer, Snapshot, SnapshotValue, snapshot_value};
use crate::log::{LogBus, now_us};
use crate::registry::NodeRegistry;
use crate::vars::VarType;
use arc_swap::ArcSwap;
use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use vf_abi::{VfHostApi, VfProcessCtx, VfValue, VfValueTag};

const ENGINE_RATE_HZ: f32 = 100.0;

pub(crate) enum EngineCommand {
    SwapPlan(ExecPlan, Graph),
    SetParam(NodeId, String, serde_json::Value),
    Start,
    Stop,
    Shutdown,
}

struct HostState {
    log: LogBus,
    wake: Arc<(Mutex<bool>, Condvar)>,
}

struct NodeRuntime {
    instance: NodeInstance,
    out_bufs: Vec<PortBuffer>,
    out_vals: Vec<VfValue>,
    in_vals: Vec<VfValue>,
}

struct EngineInner {
    registry: NodeRegistry,
    host_api: Box<VfHostApi>,
    /// Kept so `host_api.user_data` stays valid for plugin callbacks.
    #[allow(dead_code)]
    host_state: Box<HostState>,
    nodes: HashMap<NodeId, NodeRuntime>,
    plan: ExecPlan,
    graph: Graph,
    running: bool,
    tick: u64,
    drops: u64,
    last_now: u64,
}

pub struct EngineHandle {
    pub snapshot: Arc<ArcSwap<Snapshot>>,
    cmd_tx: crossbeam_channel::Sender<EngineCommand>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl EngineHandle {
    fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
        self.wake_now();
    }

    fn wake_now(&self) {
        let (lock, cv) = &*self.wake;
        *lock.lock() = true;
        cv.notify_one();
    }

    pub fn start(&self) {
        self.set_snapshot_running(true);
        self.send(EngineCommand::Start);
    }

    pub fn stop(&self) {
        self.set_snapshot_running(false);
        self.send(EngineCommand::Stop);
    }

    fn set_snapshot_running(&self, running: bool) {
        let cur = self.snapshot.load_full();
        if cur.running == running {
            return;
        }
        let mut snap = (*cur).clone();
        snap.running = running;
        self.snapshot.store(Arc::new(snap));
    }

    pub fn shutdown(&self) {
        self.send(EngineCommand::Shutdown);
        if let Some(h) = self.thread.lock().take() {
            let _ = h.join();
        }
    }

    pub fn swap_graph(&self, plan: ExecPlan, graph: Graph) {
        self.send(EngineCommand::SwapPlan(plan, graph));
    }

    pub fn set_param(&self, id: NodeId, key: String, value: serde_json::Value) {
        self.send(EngineCommand::SetParam(id, key, value));
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.load_full()
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn spawn_engine(registry: NodeRegistry, log: LogBus) -> EngineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
    let wake = Arc::new((Mutex::new(false), Condvar::new()));

    let host_state = Box::new(HostState {
        log: log.clone(),
        wake: wake.clone(),
    });
    let host_ptr = host_state.as_ref() as *const HostState as *mut c_void;
    let host_api = Box::new(VfHostApi {
        user_data: host_ptr,
        log: host_log,
        wake: host_wake,
        now_us: host_now_us,
    });

    let snap = snapshot.clone();
    let wake_t = wake.clone();
    let thread = thread::Builder::new()
        .name("vf-engine".into())
        .spawn(move || {
            let mut inner = EngineInner {
                registry,
                host_api,
                host_state,
                nodes: HashMap::new(),
                plan: ExecPlan {
                    order: vec![],
                    bindings: HashMap::new(),
                },
                graph: Graph::default(),
                running: false,
                tick: 0,
                drops: 0,
                last_now: now_us(),
            };
            engine_loop(&mut inner, cmd_rx, snap, wake_t);
        })
        .expect("spawn engine thread");

    EngineHandle {
        snapshot,
        cmd_tx,
        wake,
        thread: Mutex::new(Some(thread)),
    }
}

fn engine_loop(
    inner: &mut EngineInner,
    cmd_rx: crossbeam_channel::Receiver<EngineCommand>,
    snapshot: Arc<ArcSwap<Snapshot>>,
    wake: Arc<(Mutex<bool>, Condvar)>,
) {
    loop {
        let was_running = inner.running;
        while let Ok(cmd) = cmd_rx.try_recv() {
            if !handle_cmd(inner, cmd) {
                stop_all(inner);
                return;
            }
        }
        if inner.running {
            run_tick(inner);
            publish(inner, &snapshot);
        } else if was_running {
            publish(inner, &snapshot);
        }
        let period = Duration::from_secs_f32(1.0 / ENGINE_RATE_HZ);
        let (lock, cv) = &*wake;
        let mut signaled = lock.lock();
        if !*signaled {
            let _ = cv.wait_for(&mut signaled, period);
        }
        *signaled = false;
    }
}

fn handle_cmd(inner: &mut EngineInner, cmd: EngineCommand) -> bool {
    match cmd {
        EngineCommand::Shutdown => return false,
        EngineCommand::Start => {
            inner.running = true;
            start_all(inner);
            inner.host_state.log.log(2, None, "engine running");
        }
        EngineCommand::Stop => {
            inner.running = false;
            stop_all(inner);
            inner.host_state.log.log(2, None, "engine stopped");
        }
        EngineCommand::SetParam(id, key, value) => {
            if let Some(n) = inner.nodes.get_mut(&id) {
                if let Err(e) = n.instance.set_param(&key, &value) {
                    inner
                        .host_state
                        .log
                        .log(1, Some(id.0), format!("set_param failed: {e}"));
                }
            }
            if let Some(gn) = inner.graph.node_mut(id) {
                if let Some(obj) = gn.params.as_object_mut() {
                    obj.insert(key, value);
                }
            }
        }
        EngineCommand::SwapPlan(plan, graph) => {
            apply_plan(inner, plan, graph);
        }
    }
    true
}

fn apply_plan(inner: &mut EngineInner, plan: ExecPlan, graph: Graph) {
    let was_running = inner.running;
    let keep: std::collections::HashSet<NodeId> = plan.order.iter().copied().collect();
    inner.nodes.retain(|id, rt| {
        if keep.contains(id) {
            if let Some(b) = plan.bindings.get(id) {
                if rt.instance.type_id() == b.type_id {
                    return true;
                }
            }
        }
        if was_running {
            rt.instance.stop();
        }
        false
    });
    for id in &plan.order {
        if inner.nodes.contains_key(id) {
            continue;
        }
        let Some(binding) = plan.bindings.get(id) else {
            continue;
        };
        let Some(ty) = inner.registry.get(&binding.type_id).cloned() else {
            continue;
        };
        let gn = graph.node(*id);
        let params = gn
            .map(|n| n.params.clone())
            .unwrap_or(serde_json::json!({}));
        match NodeInstance::create(&ty, inner.host_api.as_ref(), id.0, &params) {
            Ok(mut inst) => {
                if let Some(state) = gn.and_then(|n| n.state.clone()) {
                    let _ = inst.set_state(&state);
                }
                if was_running {
                    if let Err(e) = inst.start() {
                        inner
                            .host_state
                            .log
                            .log(1, Some(id.0), format!("start failed: {e}"));
                    }
                }
                let mut out_bufs: Vec<PortBuffer> = ty
                    .outputs
                    .iter()
                    .map(|p| PortBuffer::from_port(p.value_tag(), &p.schema, p.capacity))
                    .collect();
                let out_vals: Vec<VfValue> = out_bufs
                    .iter_mut()
                    .zip(ty.outputs.iter())
                    .map(|(b, p)| b.as_value(p.value_tag()))
                    .collect();
                inner.nodes.insert(
                    *id,
                    NodeRuntime {
                        instance: inst,
                        out_bufs,
                        out_vals,
                        in_vals: vec![VfValue::empty(); binding.n_in],
                    },
                );
                if let Some(rt) = inner.nodes.get_mut(id) {
                    refresh_runtime_ports(rt);
                }
            }
            Err(e) => {
                inner
                    .host_state
                    .log
                    .log(0, Some(id.0), format!("failed to create node: {e}"))
            }
        }
    }
    inner.plan = plan;
    inner.graph = graph;
}

fn start_all(inner: &mut EngineInner) {
    for (id, rt) in inner.nodes.iter_mut() {
        if let Err(e) = rt.instance.start() {
            inner
                .host_state
                .log
                .log(1, Some(id.0), format!("start failed: {e}"));
        }
    }
}

fn stop_all(inner: &mut EngineInner) {
    for rt in inner.nodes.values_mut() {
        rt.instance.stop();
    }
}

fn refresh_runtime_ports(rt: &mut NodeRuntime) {
    let tags: Vec<VfValueTag> = rt
        .instance
        .node_type()
        .outputs
        .iter()
        .map(|p| p.value_tag())
        .collect();
    for (i, buf) in rt.out_bufs.iter_mut().enumerate() {
        if let Some(tag) = tags.get(i) {
            if i < rt.out_vals.len() {
                rt.out_vals[i] = buf.as_value(*tag);
            }
        }
    }
}

fn run_tick(inner: &mut EngineInner) {
    let now = now_us();
    let dt = now.saturating_sub(inner.last_now).max(1);
    inner.last_now = now;
    inner.tick += 1;
    let ctx = VfProcessCtx {
        tick: inner.tick,
        dt_us: dt,
        now_us: now,
    };

    // Refresh output value pointers (Vec realloc / HashMap move safety).
    for rt in inner.nodes.values_mut() {
        refresh_runtime_ports(rt);
    }

    let order = inner.plan.order.clone();
    for id in order {
        let Some(binding) = inner.plan.bindings.get(&id).cloned() else {
            continue;
        };
        if let Some((is_get, vty)) = VarType::from_type_id(&binding.type_id) {
            tick_builtin(inner, id, &binding, is_get, vty);
            continue;
        }
        // Assemble inputs from source outputs.
        let mut ins = vec![VfValue::empty(); binding.n_in];
        for (i, src) in binding.inputs.iter().enumerate() {
            if let Some(p) = src {
                if let Some(src_rt) = inner.nodes.get(&p.node) {
                    if let Some(v) = src_rt.out_vals.get(p.port as usize) {
                        ins[i] = *v;
                    }
                }
            }
        }
        for (key, from) in &binding.param_inputs {
            let Some(v) = inner
                .nodes
                .get(&from.node)
                .and_then(|rt| rt.out_vals.get(from.port as usize))
                .copied()
            else {
                continue;
            };
            let json = vf_to_json(v);
            if let Some(rt) = inner.nodes.get_mut(&id) {
                let _ = rt.instance.set_param(key, &json);
            }
            if let Some(gn) = inner.graph.node_mut(id) {
                if let Some(obj) = gn.params.as_object_mut() {
                    obj.insert(key.clone(), json);
                }
            }
        }
        let Some(rt) = inner.nodes.get_mut(&id) else {
            continue;
        };
        rt.in_vals = ins;
        let st = rt.instance.process(&ctx, &rt.in_vals, &mut rt.out_vals);
        if st == vf_abi::VF_ERR {
            inner.drops += 1;
            inner
                .host_state
                .log
                .log(3, Some(id.0), "node process error / disabled");
        }
    }
}

fn tick_builtin(
    inner: &mut EngineInner,
    id: NodeId,
    binding: &crate::graph::NodeBinding,
    is_get: bool,
    vty: VarType,
) {
    let name = inner
        .graph
        .node(id)
        .and_then(|n| n.params.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if is_get {
        let value = inner
            .graph
            .var(&name)
            .map(|v| v.value.clone())
            .unwrap_or_else(|| vty.default_value());
        if let Some(rt) = inner.nodes.get_mut(&id) {
            write_builtin_out(rt, vty, &value);
        }
        return;
    }
    let mut incoming = VfValue::empty();
    if let Some(Some(p)) = binding.inputs.first() {
        if let Some(src_rt) = inner.nodes.get(&p.node) {
            if let Some(v) = src_rt.out_vals.get(p.port as usize) {
                incoming = *v;
            }
        }
    }
    if incoming.tag != VfValueTag::Empty {
        let json = vf_to_json(incoming);
        if let Some(var) = inner.graph.var_mut(&name) {
            var.value = json.clone();
        }
        if let Some(rt) = inner.nodes.get_mut(&id) {
            write_builtin_out(rt, vty, &json);
        }
    } else {
        let value = inner
            .graph
            .var(&name)
            .map(|v| v.value.clone())
            .unwrap_or_else(|| vty.default_value());
        if let Some(rt) = inner.nodes.get_mut(&id) {
            write_builtin_out(rt, vty, &value);
        }
    }
}

fn write_builtin_out(rt: &mut NodeRuntime, vty: VarType, value: &serde_json::Value) {
    if matches!(vty, VarType::String) {
        let s = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if let Some(PortBuffer::Bytes { data, .. }) = rt.out_bufs.get_mut(0) {
            data.clear();
            data.extend_from_slice(s.as_bytes());
        }
        if let Some(buf) = rt.out_bufs.get_mut(0) {
            let v = buf.as_value(VfValueTag::Bytes);
            if let Some(slot) = rt.out_vals.get_mut(0) {
                *slot = v;
            }
        }
        return;
    }
    if let Some(slot) = rt.out_vals.get_mut(0) {
        *slot = json_to_vf(value, vty.tag());
    }
}

fn vf_to_json(v: VfValue) -> serde_json::Value {
    match v.tag {
        VfValueTag::Float => serde_json::json!(v.as_float().unwrap_or(0.0)),
        VfValueTag::Int => serde_json::json!(v.as_int().unwrap_or(0)),
        VfValueTag::Bool => serde_json::json!(v.as_bool().unwrap_or(false)),
        VfValueTag::Bytes => {
            let b = unsafe { v.payload.bytes };
            if b.ptr.is_null() || b.len == 0 {
                serde_json::json!("")
            } else {
                let sl = unsafe { std::slice::from_raw_parts(b.ptr, b.len as usize) };
                serde_json::Value::String(String::from_utf8_lossy(sl).into_owned())
            }
        }
        _ => serde_json::Value::Null,
    }
}

fn json_to_vf(v: &serde_json::Value, tag: VfValueTag) -> VfValue {
    match tag {
        VfValueTag::Float => {
            let n = v
                .as_f64()
                .or_else(|| v.as_i64().map(|i| i as f64))
                .unwrap_or(0.0);
            VfValue::float(n as f32)
        }
        VfValueTag::Int => {
            let n = v
                .as_i64()
                .or_else(|| v.as_f64().map(|f| f as i64))
                .unwrap_or(0);
            VfValue::int(n)
        }
        VfValueTag::Bool => VfValue::boolean(v.as_bool().unwrap_or(false)),
        _ => VfValue::empty(),
    }
}

fn publish(inner: &EngineInner, snapshot: &ArcSwap<Snapshot>) {
    let mut nodes = HashMap::new();
    for (id, rt) in &inner.nodes {
        let st = rt.instance.status();
        let msg = {
            let end = st
                .message
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(st.message.len());
            String::from_utf8_lossy(&st.message[..end]).into_owned()
        };
        let outputs: Vec<SnapshotValue> = rt.out_vals.iter().map(snapshot_value).collect();
        nodes.insert(
            id.0,
            crate::instance::NodeSnap {
                status_level: st.level,
                status_text: msg,
                outputs,
                disabled: rt.instance.disabled,
                state: None,
            },
        );
    }
    snapshot.store(Arc::new(Snapshot {
        tick: inner.tick,
        dt_us: inner.last_now,
        running: inner.running,
        drops: inner.drops,
        last_tick_us: inner.last_now,
        nodes,
    }));
}

unsafe extern "C" fn host_log(
    user: *mut c_void,
    level: u32,
    node_handle: u64,
    msg: *const std::ffi::c_char,
) {
    if user.is_null() {
        return;
    }
    let state = unsafe { &*(user as *const HostState) };
    let message = if msg.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(msg) }
            .to_string_lossy()
            .into_owned()
    };
    let node = if node_handle == 0 {
        None
    } else {
        Some(node_handle)
    };
    state.log.log(level, node, message);
}

unsafe extern "C" fn host_wake(user: *mut c_void, _node_handle: u64) {
    if user.is_null() {
        return;
    }
    let state = unsafe { &*(user as *const HostState) };
    let (lock, cv) = &*state.wake;
    *lock.lock() = true;
    cv.notify_one();
}

unsafe extern "C" fn host_now_us(_user: *mut c_void) -> u64 {
    now_us()
}
