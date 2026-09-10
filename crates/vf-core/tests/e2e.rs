//! Headless synthetic Unified → OSC, if plugin cdylibs are present.

use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use vf_core::{Graph, GraphEdge, GraphNode, NodeId, PortRef, Session};

fn plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dirs.push(manifest.join("../../target/debug"));
    dirs.push(manifest.join("../../target/debug/plugins"));
    dirs.push(manifest.join("../../plugins"));
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        dirs.push(PathBuf::from(&td).join("debug"));
        dirs.push(PathBuf::from(&td).join("debug/plugins"));
    }
    dirs
}

#[test]
fn loads_example_plugins() {
    let session = Session::boot(&plugin_dirs(), None);
    let types: Vec<_> = session
        .registry
        .all()
        .iter()
        .map(|t| t.type_id.clone())
        .collect();
    eprintln!(
        "loaded plugins={} types={types:?}",
        session.host.plugins().len()
    );
    assert!(
        types.iter().any(|t| t == "pico.synthetic"),
        "pico.synthetic missing; dirs={:?} types={types:?}",
        plugin_dirs()
    );
    assert!(types.iter().any(|t| t == "unified.one_euro"));
    assert!(types.iter().any(|t| t == "vrc.osc_output"));
}

#[test]
fn synthetic_to_osc_if_plugins_built() {
    let session = Session::boot(&plugin_dirs(), None);
    let types: Vec<_> = session
        .registry
        .all()
        .iter()
        .map(|t| t.type_id.clone())
        .collect();
    if !types.iter().any(|t| t == "pico.synthetic") || !types.iter().any(|t| t == "vrc.osc_output") {
        eprintln!("skip e2e: plugins not loaded ({types:?})");
        return;
    }

    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let port = sock.local_addr().unwrap().port();

    let mut g = Graph::default();
    g.nodes = vec![
        GraphNode {
            id: NodeId(1),
            type_id: "pico.synthetic".into(),
            x: 0.0,
            y: 0.0,
            params: serde_json::json!({"hz": 2.0, "amplitude": 1.0}),
            state: None,
            missing: false,
        },
        GraphNode {
            id: NodeId(2),
            type_id: "vrc.osc_output".into(),
            x: 400.0,
            y: 0.0,
            params: serde_json::json!({
                "host": "127.0.0.1",
                "port": port,
                "prefix": "/avatar/parameters/",
                "send_all": true,
                "oscquery": false,
                "binary_bits": 0
            }),
            state: None,
            missing: false,
        },
    ];
    g.edges = vec![GraphEdge {
        from: PortRef {
            node: NodeId(1),
            port: 0,
        },
        to: PortRef {
            node: NodeId(2),
            port: 0,
        },
    }];
    g.next_id = 3;
    *session.graph.lock() = g;
    session.recompile();
    session.engine.start();
    std::thread::sleep(Duration::from_millis(50));

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut buf = [0u8; 8192];
    let mut got = false;
    while Instant::now() < deadline {
        if sock.recv_from(&mut buf).is_ok() {
            got = true;
            break;
        }
    }
    session.engine.stop();
    assert!(got, "expected OSC datagrams from vrc.osc_output");
}
