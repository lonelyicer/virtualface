use crate::error::{CoreError, Result};
use crate::registry::NodeRegistry;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use vf_abi::ports_compatible;
use vf_sdk::defaults_from_schema;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortRef {
    pub node: NodeId,
    pub port: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub type_id: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "object_default")]
    pub params: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
}

fn object_default() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: PortRef,
    pub to: PortRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graph {
    pub version: u32,
    #[serde(default = "default_rate")]
    pub rate_hz: f32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    #[serde(default)]
    pub next_id: u64,
}

fn default_rate() -> f32 {
    100.0
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            version: 1,
            rate_hz: 100.0,
            nodes: vec![],
            edges: vec![],
            next_id: 1,
        }
    }
}

impl Graph {
    pub fn node(&self, id: NodeId) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn add_node(&mut self, type_id: &str, x: f32, y: f32, registry: &NodeRegistry) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        let (params, missing) = if let Some(ty) = registry.get(type_id) {
            (defaults_from_schema(&ty.params), false)
        } else {
            (serde_json::json!({}), true)
        };
        self.nodes.push(GraphNode {
            id,
            type_id: type_id.into(),
            x,
            y,
            params,
            state: None,
            missing,
        });
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from.node != id && e.to.node != id);
    }

    pub fn can_connect(&self, from: PortRef, to: PortRef, registry: &NodeRegistry) -> Result<()> {
        if from.node == to.node {
            return Err(CoreError::Graph("cannot connect a node to itself".into()));
        }
        let src = self
            .node(from.node)
            .ok_or_else(|| CoreError::Graph("unknown source node".into()))?;
        let dst = self
            .node(to.node)
            .ok_or_else(|| CoreError::Graph("unknown dest node".into()))?;
        let src_ty = registry
            .get(&src.type_id)
            .ok_or_else(|| CoreError::UnknownType(src.type_id.clone()))?;
        let dst_ty = registry
            .get(&dst.type_id)
            .ok_or_else(|| CoreError::UnknownType(dst.type_id.clone()))?;
        let sp = src_ty
            .outputs
            .get(from.port as usize)
            .ok_or_else(|| CoreError::Graph("unknown output port".into()))?;
        let dp = dst_ty
            .inputs
            .get(to.port as usize)
            .ok_or_else(|| CoreError::Graph("unknown input port".into()))?;
        if !ports_compatible(
            sp.value_tag(),
            Some(&sp.schema),
            dp.value_tag(),
            Some(&dp.schema),
        ) {
            return Err(CoreError::Graph(format!(
                "incompatible ports {} -> {}",
                sp.name, dp.name
            )));
        }
        if self.edges.iter().any(|e| e.to == to) {
            return Err(CoreError::Graph("input port already connected".into()));
        }
        // Cycle check: temporarily add and toposort.
        let mut tmp = self.clone();
        tmp.edges.push(GraphEdge { from, to });
        tmp.toposort(registry)?;
        Ok(())
    }

    pub fn connect(&mut self, from: PortRef, to: PortRef, registry: &NodeRegistry) -> Result<()> {
        self.can_connect(from, to, registry)?;
        self.edges.push(GraphEdge { from, to });
        Ok(())
    }

    pub fn disconnect(&mut self, to: PortRef) {
        self.edges.retain(|e| e.to != to);
    }

    pub fn disconnect_edge(&mut self, from: PortRef, to: PortRef) {
        self.edges.retain(|e| !(e.from == from && e.to == to));
    }

    pub fn toposort(&self, _registry: &NodeRegistry) -> Result<Vec<NodeId>> {
        use petgraph::algo::toposort;
        use petgraph::graph::DiGraph;

        let mut g = DiGraph::<NodeId, ()>::new();
        let mut idx = HashMap::new();
        for n in &self.nodes {
            idx.insert(n.id, g.add_node(n.id));
        }
        for e in &self.edges {
            if let (Some(&a), Some(&b)) = (idx.get(&e.from.node), idx.get(&e.to.node)) {
                g.add_edge(a, b, ());
            }
        }
        match toposort(&g, None) {
            Ok(order) => Ok(order.into_iter().map(|i| g[i]).collect()),
            Err(_) => Err(CoreError::Graph("graph contains a cycle".into())),
        }
    }

    pub fn incoming(&self, id: NodeId) -> impl Iterator<Item = &GraphEdge> {
        self.edges.iter().filter(move |e| e.to.node == id)
    }

    pub fn mark_missing(&mut self, registry: &NodeRegistry) {
        for n in &mut self.nodes {
            n.missing = registry.get(&n.type_id).is_none();
        }
    }

    pub fn used_ids(&self) -> HashSet<NodeId> {
        self.nodes.iter().map(|n| n.id).collect()
    }
}

#[derive(Clone, Debug)]
pub struct NodeBinding {
    pub type_id: String,
    pub inputs: Vec<Option<PortRef>>,
    pub n_in: usize,
    pub n_out: usize,
}

#[derive(Clone, Debug)]
pub struct ExecPlan {
    pub order: Vec<NodeId>,
    pub bindings: HashMap<NodeId, NodeBinding>,
    pub rate_hz: f32,
}

impl Graph {
    pub fn compile(&self, registry: &NodeRegistry) -> Result<ExecPlan> {
        let order = self.toposort(registry)?;
        let mut bindings = HashMap::new();
        for n in &self.nodes {
            if n.missing || registry.get(&n.type_id).is_none() {
                continue;
            }
            let ty = registry.get(&n.type_id).unwrap();
            let mut inputs = vec![None; ty.inputs.len()];
            for e in self.incoming(n.id) {
                if (e.to.port as usize) < inputs.len() {
                    inputs[e.to.port as usize] = Some(e.from);
                }
            }
            bindings.insert(
                n.id,
                NodeBinding {
                    type_id: n.type_id.clone(),
                    inputs,
                    n_in: ty.inputs.len(),
                    n_out: ty.outputs.len(),
                },
            );
        }
        Ok(ExecPlan {
            order: order
                .into_iter()
                .filter(|id| bindings.contains_key(id))
                .collect(),
            bindings,
            rate_hz: self.rate_hz.max(1.0),
        })
    }
}

pub fn graph_from_json(s: &str) -> Result<Graph> {
    let mut g: Graph = serde_json::from_str(s)?;
    if g.next_id == 0 {
        g.next_id = g.nodes.iter().map(|n| n.id.0).max().unwrap_or(0) + 1;
    }
    Ok(g)
}

pub fn graph_to_json(g: &Graph) -> Result<String> {
    Ok(serde_json::to_string_pretty(g)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{NodeRegistry, NodeType};
    use vf_abi::{VfNodeVTable, VfValueTag};
    use vf_sdk::Category;

    fn dummy_vtable() -> VfNodeVTable {
        unsafe extern "C" fn nop_destroy(_: *mut std::ffi::c_void) {}
        unsafe extern "C" fn nop_start(_: *mut std::ffi::c_void) -> i32 {
            0
        }
        unsafe extern "C" fn nop_stop(_: *mut std::ffi::c_void) {}
        unsafe extern "C" fn nop_process(
            _: *mut std::ffi::c_void,
            _: *const vf_abi::VfProcessCtx,
            _: *const vf_abi::VfValue,
            _: u32,
            _: *mut vf_abi::VfValue,
            _: u32,
        ) -> i32 {
            0
        }
        unsafe extern "C" fn nop_param(
            _: *mut std::ffi::c_void,
            _: *const std::ffi::c_char,
            _: *const std::ffi::c_char,
        ) -> i32 {
            0
        }
        unsafe extern "C" fn nop_get(_: *mut std::ffi::c_void, _: *mut u8, _: usize) -> usize {
            0
        }
        unsafe extern "C" fn nop_set(_: *mut std::ffi::c_void, _: *const std::ffi::c_char) -> i32 {
            0
        }
        unsafe extern "C" fn nop_status(_: *mut std::ffi::c_void, _: *mut vf_abi::VfNodeStatus) {}
        VfNodeVTable {
            destroy: nop_destroy,
            start: nop_start,
            stop: nop_stop,
            process: nop_process,
            set_param: nop_param,
            get_state: nop_get,
            set_state: nop_set,
            status: nop_status,
        }
    }

    unsafe extern "C" fn dummy_create(
        _: *const vf_abi::VfHostApi,
        _: u64,
        _: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void {
        1 as *mut _
    }

    fn ty(id: &str, cat: Category, ins: &[VfValueTag], outs: &[VfValueTag]) -> NodeType {
        NodeType {
            plugin_id: "test".into(),
            plugin_name: "test".into(),
            type_id: id.into(),
            display_name: id.into(),
            category: cat,
            flags: 0,
            inputs: ins
                .iter()
                .enumerate()
                .map(|(i, t)| crate::registry::PortType {
                    name: format!("in{i}"),
                    tag: *t as u32,
                    schema: String::new(),
                    capacity: 0,
                })
                .collect(),
            outputs: outs
                .iter()
                .enumerate()
                .map(|(i, t)| crate::registry::PortType {
                    name: format!("out{i}"),
                    tag: *t as u32,
                    schema: String::new(),
                    capacity: 0,
                })
                .collect(),
            params: vec![],
            create: dummy_create,
            vtable: dummy_vtable(),
        }
    }

    #[test]
    fn compile_linear_and_reject_cycle() {
        let mut reg = NodeRegistry::new();
        reg.register(ty("src", Category::Input, &[], &[VfValueTag::Float]));
        reg.register(ty(
            "proc",
            Category::Process,
            &[VfValueTag::Float],
            &[VfValueTag::Float],
        ));
        let mut g = Graph::default();
        let a = g.add_node("src", 0.0, 0.0, &reg);
        let b = g.add_node("proc", 200.0, 0.0, &reg);
        g.connect(
            PortRef { node: a, port: 0 },
            PortRef { node: b, port: 0 },
            &reg,
        )
        .unwrap();
        let plan = g.compile(&reg).unwrap();
        assert_eq!(plan.order, vec![a, b]);

        // cycle: would need proc -> src, src has no input. Use two procs.
        let mut g2 = Graph::default();
        g2.add_node("proc", 0.0, 0.0, &reg);
        let p1 = NodeId(1);
        let p2 = g2.add_node("proc", 0.0, 0.0, &reg);
        g2.edges.push(GraphEdge {
            from: PortRef { node: p1, port: 0 },
            to: PortRef { node: p2, port: 0 },
        });
        g2.edges.push(GraphEdge {
            from: PortRef { node: p2, port: 0 },
            to: PortRef { node: p1, port: 0 },
        });
        assert!(g2.toposort(&reg).is_err());
    }

    #[test]
    fn roundtrip_json() {
        let g = Graph {
            version: 1,
            rate_hz: 90.0,
            next_id: 3,
            nodes: vec![GraphNode {
                id: NodeId(1),
                type_id: "src".into(),
                x: 10.0,
                y: 20.0,
                params: serde_json::json!({"port": 1}),
                state: None,
                missing: false,
            }],
            edges: vec![],
        };
        let s = graph_to_json(&g).unwrap();
        let g2 = graph_from_json(&s).unwrap();
        assert_eq!(g2.rate_hz, 90.0);
        assert_eq!(g2.nodes[0].type_id, "src");
    }
}
