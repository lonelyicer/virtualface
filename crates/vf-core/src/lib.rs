//! VirtualFace host: plugin loader, graph compiler, execution engine.

pub mod builtins;
pub mod engine;
pub mod error;
pub mod graph;
pub mod instance;
pub mod log;
pub mod plugin;
pub mod prefs;
pub mod registry;
pub mod vars;

pub use engine::{EngineCommand, EngineHandle, spawn_engine};
pub use error::{CoreError, Result};
pub use graph::{
    ExecPlan, Graph, GraphEdge, GraphNode, NodeId, PortRef, graph_from_json, graph_to_json,
};
pub use instance::{NodeSnap, Snapshot, SnapshotValue};
pub use log::{LogBus, LogLine, format_ts, level_name, now_us, tracing_from_bus};
pub use plugin::{LoadedPlugin, PluginHost, default_plugin_dirs};
pub use prefs::{
    absolute_path, config_dir, graph_display_name, last_graph_path, load_locale, remember_last_graph,
    save_locale, with_graph_extension,
};
pub use registry::{NodeRegistry, NodeType, PortType};
pub use vars::{GraphVar, VarType};

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared application state used by both the GUI and headless runner.
///
/// `engine` is declared before `host` so it is dropped first: node `destroy`
/// trampolines must run while plugin libraries are still mapped.
pub struct Session {
    pub engine: EngineHandle,
    pub registry: NodeRegistry,
    pub graph: Arc<Mutex<Graph>>,
    pub load_errors: Vec<String>,
    pub graph_path: Mutex<String>,
    pub host: PluginHost,
}

impl Session {
    pub fn boot(plugin_dirs: &[PathBuf], graph: Option<Graph>) -> Self {
        Self::boot_with_log(plugin_dirs, graph, LogBus::new())
    }

    pub fn boot_with_log(plugin_dirs: &[PathBuf], graph: Option<Graph>, log: LogBus) -> Self {
        let mut host = PluginHost::new(log.clone());
        let mut registry = NodeRegistry::new();
        crate::builtins::register_builtins(&mut registry);
        let load_errors = host.scan_and_load(plugin_dirs, &mut registry);
        for e in &load_errors {
            log.log(1, None, e.clone());
        }
        log.log(
            2,
            None,
            format!(
                "session ready: {} plugins, {} node types",
                host.plugins().len(),
                registry.all().len()
            ),
        );
        let mut g = graph.unwrap_or_default();
        g.mark_missing(&registry);
        let engine = spawn_engine(registry.clone(), log);
        match g.compile(&registry) {
            Ok(plan) => engine.swap_graph(plan, g.clone()),
            Err(e) => host.log.log(0, None, format!("compile: {e}")),
        }
        Self {
            engine,
            registry,
            graph: Arc::new(Mutex::new(g)),
            load_errors,
            graph_path: Mutex::new(String::new()),
            host,
        }
    }

    pub fn recompile(&self) {
        let g = self.graph.lock().clone();
        match g.compile(&self.registry) {
            Ok(plan) => self.engine.swap_graph(plan, g),
            Err(e) => self.host.log.log(0, None, format!("compile: {e}")),
        }
    }

    pub fn load_graph_str(&self, json: &str) -> Result<()> {
        let mut g = graph_from_json(json)?;
        g.mark_missing(&self.registry);
        *self.graph.lock() = g;
        self.recompile();
        Ok(())
    }

    pub fn save_graph_str(&self) -> Result<String> {
        let mut g = self.graph.lock().clone();
        let snap = self.engine.snapshot();
        for n in &mut g.nodes {
            if let Some(ns) = snap.nodes.get(&n.id.0) {
                n.state = ns.state.clone();
            }
        }
        graph_to_json(&g)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Join the engine thread and destroy node instances before `PluginHost`
        // unmaps the cdylibs those vtables live in.
        self.engine.shutdown();
    }
}
