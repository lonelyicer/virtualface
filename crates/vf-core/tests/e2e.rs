//! Load the current plugin set and exercise a built-in float source → OSC output.

use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use vf_core::{Graph, GraphVar, PortRef, Session, VarType};

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
    for expected in [
        "pico.udp_source",
        "unified.one_euro",
        "vrc.osc_output",
        "vrc.osc_raw",
    ] {
        assert!(
            session.registry.get(expected).is_some(),
            "{expected} missing; build plugin cdylibs first"
        );
    }
}

#[test]
fn float_variable_to_osc() {
    let session = Session::boot(&plugin_dirs(), None);
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let port = sock.local_addr().unwrap().port();

    let mut graph = Graph::default();
    graph.variables.push(GraphVar {
        name: "test".into(),
        ty: VarType::Float,
        value: serde_json::json!(0.375),
    });
    let source = graph.add_node("vf.get.float", 0., 0., &session.registry);
    let output = graph.add_node("vrc.osc_raw", 400., 0., &session.registry);
    graph.node_mut(source).unwrap().params = serde_json::json!({"name": "test"});
    graph.node_mut(output).unwrap().params = serde_json::json!({
        "host": "127.0.0.1", "port": port, "address": "/virtualface/test",
    });
    graph
        .connect(
            PortRef {
                node: source,
                port: 0,
            },
            PortRef {
                node: output,
                port: 0,
            },
            &session.registry,
        )
        .unwrap();
    *session.graph.lock() = graph;
    session.recompile();
    session.engine.start();

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut buf = [0u8; 8192];
    let mut received = None;
    while Instant::now() < deadline {
        if let Ok((len, _)) = sock.recv_from(&mut buf) {
            received = Some(rosc::decoder::decode_udp(&buf[..len]).unwrap().1);
            break;
        }
    }
    session.engine.stop();
    assert_eq!(
        received,
        Some(rosc::OscPacket::Message(rosc::OscMessage {
            addr: "/virtualface/test".into(),
            args: vec![rosc::OscType::Float(0.375)],
        }))
    );
}
