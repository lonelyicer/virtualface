use crate::graph::{ExecPlan, Graph, NodeId};
use crate::instance::{NodeInstance, PortBuffer, Snapshot, SnapshotValue, snapshot_value};
use crate::log::{LogBus, now_us};
use crate::registry::NodeRegistry;
use arc_swap::ArcSwap;
use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, warn};
use vf_abi::{
    VF_LOG_DEBUG, VF_LOG_ERROR, VF_LOG_INFO, VF_LOG_WARN, VfHostApi, VfProcessCtx, VfValue,
    VfValueTag,
};

pub enum EngineCommand {
    SwapPlan(ExecPlan, Graph),
    SetParam(NodeId, String, serde_json::Value),
    Start,
    Stop,
    Shutdown,
    TickOnce,
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
    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
        self.wake_now();
    }

    pub fn wake_now(&self) {
        let (lock, cv) = &*self.wake;
        *lock.lock() = true;
        cv.notify_one();
    }

    pub fn start(&self) {
        self.send(EngineCommand::Start);
    }

    pub fn stop(&self) {
        self.send(EngineCommand::Stop);
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

    pub fn tick_once(&self) {
        self.send(EngineCommand::TickOnce);
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
                    rate_hz: 100.0,
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
        while let Ok(cmd) = cmd_rx.try_recv() {
            if !handle_cmd(inner, cmd) {
                stop_all(inner);
                return;
            }
        }
        let mut force = false;
        if inner.running {
            run_tick(inner);
            publish(inner, &snapshot);
        }
        let period = Duration::from_secs_f32(1.0 / inner.plan.rate_hz.max(1.0));
        let (lock, cv) = &*wake;
        let mut signaled = lock.lock();
        if !*signaled {
            let _ = cv.wait_for(&mut signaled, period);
        }
        if *signaled {
            force = true;
            *signaled = false;
        }
        drop(signaled);
        if force && inner.running {
            // extra tick already handled next loop; drain commands first
        }
        let _ = force;
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
        EngineCommand::TickOnce => {
            run_tick(inner);
        }
        EngineCommand::SetParam(id, key, value) => {
            if let Some(n) = inner.nodes.get_mut(&id) {
                if let Err(e) = n.instance.set_param(&key, &value) {
                    warn!(node = id.0, error = %e, "set_param failed");
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
                        warn!(node = id.0, error = %e, "start failed");
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
            Err(e) => error!(node = id.0, error = %e, "failed to create node"),
        }
    }
    inner.plan = plan;
    inner.graph = graph;
}

fn start_all(inner: &mut EngineInner) {
    for (id, rt) in inner.nodes.iter_mut() {
        if let Err(e) = rt.instance.start() {
            warn!(node = id.0, error = %e, "start failed");
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
        let Some(rt) = inner.nodes.get_mut(&id) else {
            continue;
        };
        rt.in_vals = ins;
        let st = rt.instance.process(&ctx, &rt.in_vals, &mut rt.out_vals);
        if st == vf_abi::VF_ERR {
            inner.drops += 1;
            debug!(node = id.0, "node process error / disabled");
        }
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
    match level {
        VF_LOG_ERROR => tracing::error!(node = node_handle, "{message}"),
        VF_LOG_WARN => tracing::warn!(node = node_handle, "{message}"),
        VF_LOG_INFO => tracing::info!(node = node_handle, "{message}"),
        VF_LOG_DEBUG => tracing::debug!(node = node_handle, "{message}"),
        _ => tracing::trace!(node = node_handle, "{message}"),
    }
    state.log.log(level, Some(node_handle), message);
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

pub fn collect_states(graph: &mut Graph, handle: &EngineHandle) {
    // Engine owns instances; we cannot reach them from the handle except via commands.
    // Persist happens on swap: graph already holds last known params; states are pulled
    // on demand by the UI before save through a dedicated snapshot field later.
    let _ = (graph, handle);
}
