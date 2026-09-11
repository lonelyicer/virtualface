use crate::oscquery::{OscQueryHub, QuerySnapshot, gaze_pitch_yaw};
use crate::packet::encode_messages;
use crate::params::{binary_magnitude, derive_v2, encode_binary};
use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use vf_abi::VfUnifiedFrame;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_bool, param_i64, param_string,
};

const FULL_REFRESH_US: u64 = 1_000_000;

struct ParamRoute {
    address: String,
    bits: u32,
    // None is the sign bit; Some(mask) is a magnitude bit.
    binary: Vec<(String, Option<u32>)>,
}

fn resolve_param(
    name: &str,
    prefix: &str,
    bits: i64,
    snap: Option<&QuerySnapshot>,
) -> Option<ParamRoute> {
    let address = match snap {
        Some(s) => s.target_addr(name, prefix)?,
        None => format!("{prefix}{name}"),
    };
    let bits = if bits > 0 {
        bits as u32
    } else {
        snap.map(|s| s.auto_bits(&address)).unwrap_or(0)
    };
    let binary = if bits == 0 {
        Vec::new()
    } else {
        encode_binary(&address, 0.0, bits)
            .into_iter()
            .enumerate()
            .filter(|(_, (addr, _))| snap.is_none_or(|s| s.has_address(addr)))
            .map(|(i, (addr, _))| (addr, i.checked_sub(1).map(|bit| 1 << bit)))
            .collect()
    };
    Some(ParamRoute {
        address,
        bits,
        binary,
    })
}

pub struct VrcOscOutput {
    plugin_host: Host,
    host_str: String,
    port: u16,
    prefix: String,
    send_all: bool,
    binary_bits: i64,
    oscquery: bool,
    query: Option<OscQueryHub>,
    sock: Option<UdpSocket>,
    addr: Option<SocketAddr>,
    routes: HashMap<String, Option<ParamRoute>>,
    routing_snapshot: Option<QuerySnapshot>,
    last: HashMap<String, f32>,
    last_binary: HashMap<String, bool>,
    last_refresh_us: Option<u64>,
    last_gaze: Option<(f32, f32, f32, f32)>,
    last_lid: Option<f32>,
    sent_active: bool,
    sent: u64,
    last_error: Option<String>,
}

impl VrcOscOutput {
    fn ensure_sock(&mut self, host: &str, port: u16) -> Result<()> {
        let want: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| vf_sdk::SdkError::other(format!("bad address: {e}")))?;
        if self.addr == Some(want) && self.sock.is_some() {
            return Ok(());
        }
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.set_nonblocking(true)?;
        self.addr = Some(want);
        self.sock = Some(sock);
        self.reset_values();
        Ok(())
    }

    fn reset_values(&mut self) {
        self.last.clear();
        self.last_binary.clear();
        self.last_gaze = None;
        self.last_lid = None;
        self.last_refresh_us = None;
        self.sent_active = false;
    }

    fn invalidate_routes(&mut self) {
        self.routes.clear();
        self.reset_values();
    }

    fn sync_routes(&mut self, snap: Option<&QuerySnapshot>) {
        let unchanged = match (&self.routing_snapshot, snap) {
            (Some(previous), Some(current)) => previous.same_routing(current),
            (None, None) => true,
            _ => false,
        };
        if !unchanged {
            self.invalidate_routes();
            self.routing_snapshot = snap.cloned();
        }
    }

    fn send_msgs(&mut self, msgs: Vec<OscMessage>) -> Result<()> {
        // Encode the complete batch before sending, so encoding errors cannot
        // leave an unknowingly partial frame on the wire.
        for bytes in encode_messages(msgs)? {
            let sock = self
                .sock
                .as_ref()
                .ok_or_else(|| vf_sdk::SdkError::other("OSC socket unavailable"))?;
            let addr = self
                .addr
                .ok_or_else(|| vf_sdk::SdkError::other("OSC target unavailable"))?;
            sock.send_to(&bytes, addr)?;
            self.sent += 1;
        }
        Ok(())
    }

    fn process_frame(
        &mut self,
        ctx: &ProcessCtx,
        frame: &VfUnifiedFrame,
        snap: Option<&QuerySnapshot>,
    ) -> Result<()> {
        let result = self.try_process_frame(ctx, frame, snap);
        if let Err(e) = &result {
            // A failed datagram (even after earlier chunks succeeded) must be
            // retried with the current complete state on the next tick.
            self.reset_values();
            self.last_error = Some(e.to_string());
        } else {
            self.last_error = None;
        }
        result
    }

    fn try_process_frame(
        &mut self,
        ctx: &ProcessCtx,
        frame: &VfUnifiedFrame,
        snap: Option<&QuerySnapshot>,
    ) -> Result<()> {
        self.sync_routes(snap);
        let host = snap
            .map(|s| s.send_host.clone())
            .unwrap_or_else(|| self.host_str.clone());
        let port = snap.map(|s| s.send_port).unwrap_or(self.port);
        self.ensure_sock(&host, port)?;
        if self
            .last_refresh_us
            .is_some_and(|last| ctx.now_us.saturating_sub(last) >= FULL_REFRESH_US)
        {
            // UDP has no acknowledgement; periodically repair lost deltas.
            self.reset_values();
        }

        let mut msgs = Vec::new();
        if !self.sent_active {
            for name in [
                "EyeTrackingActive",
                "ExpressionTrackingActive",
                "LipTrackingActive",
            ] {
                let route = self
                    .routes
                    .entry(name.into())
                    .or_insert_with(|| resolve_param(name, &self.prefix, self.binary_bits, snap));
                if let Some(route) = route {
                    msgs.push(OscMessage {
                        addr: route.address.clone(),
                        args: vec![OscType::Bool(true)],
                    });
                }
            }
            self.sent_active = true;
        }

        for (name, value) in derive_v2(frame) {
            // Cache unsuccessful matches too: most derived names are absent
            // from any one avatar's address set.
            let Some(route) = self
                .routes
                .entry(name.clone())
                .or_insert_with(|| resolve_param(&name, &self.prefix, self.binary_bits, snap))
            else {
                continue;
            };

            // Compare wire bits independently of the float epsilon. A tiny
            // change across a quantization boundary still changes a bool.
            let magnitude = binary_magnitude(value, route.bits);
            for (address, mask) in &route.binary {
                let bit = mask.map_or(value < 0.0, |mask| magnitude & mask != 0);
                if self.last_binary.get(address) != Some(&bit) {
                    msgs.push(OscMessage {
                        addr: address.clone(),
                        args: vec![OscType::Bool(bit)],
                    });
                    self.last_binary.insert(address.clone(), bit);
                }
            }
            if self
                .last
                .get(&name)
                .is_some_and(|prev| (prev - value).abs() < 1e-4)
            {
                continue;
            }
            msgs.push(OscMessage {
                addr: route.address.clone(),
                args: vec![OscType::Float(value)],
            });
            self.last.insert(name, value);
        }

        if let Some(s) = snap {
            push_native_eye(
                &mut msgs,
                frame,
                s.native_gaze,
                s.native_lid,
                &mut self.last_gaze,
                &mut self.last_lid,
            );
        }
        self.send_msgs(msgs)?;
        self.last_refresh_us.get_or_insert(ctx.now_us);
        Ok(())
    }

    fn restart_query(&mut self) {
        self.invalidate_routes();
        self.routing_snapshot = None;
        self.query = None;
        if !self.oscquery {
            return;
        }
        match OscQueryHub::start(
            self.plugin_host,
            self.send_all,
            self.host_str.clone(),
            self.port,
        ) {
            Ok(hub) => self.query = Some(hub),
            Err(e) => {
                self.last_error = Some(format!("oscquery: {e}"));
                self.plugin_host
                    .warn(&format!("OSCQuery start failed: {e}"));
            }
        }
    }
}

impl Node for VrcOscOutput {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("vrc.osc_output", "VRChat OSC Output", Category::Output)
            .input(PortDesc::unified("unified"))
            .param(ParamDef::string("host", "Host", "127.0.0.1"))
            .param(ParamDef::int("port", "Port", 9000, 1, 65535))
            .param(ParamDef::string(
                "prefix",
                "Address Prefix",
                "/avatar/parameters/",
            ))
            .param(ParamDef::boolean("send_all", "Send All Parameters", true))
            .param(ParamDef::boolean("oscquery", "OSCQuery", true))
            .param(ParamDef::int(
                "binary_bits",
                "Binary Bits (0=auto)",
                0,
                0,
                8,
            ))
    }

    fn create(host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            plugin_host: host,
            host_str: param_string(config, "host", "127.0.0.1"),
            port: param_i64(config, "port", 9000).clamp(1, 65535) as u16,
            prefix: param_string(config, "prefix", "/avatar/parameters/"),
            send_all: param_bool(config, "send_all", true),
            binary_bits: param_i64(config, "binary_bits", 0).clamp(0, 8),
            oscquery: param_bool(config, "oscquery", true),
            query: None,
            sock: None,
            addr: None,
            routes: HashMap::new(),
            routing_snapshot: None,
            last: HashMap::new(),
            last_binary: HashMap::new(),
            last_refresh_us: None,
            last_gaze: None,
            last_lid: None,
            sent_active: false,
            sent: 0,
            last_error: None,
        })
    }

    fn start(&mut self) -> Result<()> {
        self.restart_query();
        let (host, port) = self
            .query
            .as_ref()
            .map(|q| {
                let s = q.snapshot();
                (s.send_host, s.send_port)
            })
            .unwrap_or((self.host_str.clone(), self.port));
        self.ensure_sock(&host, port)?;
        self.reset_values();
        Ok(())
    }

    fn stop(&mut self) {
        self.query = None;
        self.sock = None;
        self.addr = None;
        self.routing_snapshot = None;
        self.invalidate_routes();
    }

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let frame = io.input_unified(0)?;
        let snap = self.query.as_ref().map(|q| q.snapshot());
        self.process_frame(ctx, frame, snap.as_ref())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "host" => {
                self.host_str = value.as_str().unwrap_or("127.0.0.1").into();
                if let Some(q) = &self.query {
                    q.set_manual_target(self.host_str.clone(), self.port);
                } else {
                    self.sock = None;
                }
            }
            "port" => {
                self.port = vf_sdk::json_i64(value, 9000).clamp(1, 65535) as u16;
                if let Some(q) = &self.query {
                    q.set_manual_target(self.host_str.clone(), self.port);
                } else {
                    self.sock = None;
                }
            }
            "prefix" => {
                let prefix = value.as_str().unwrap_or("/avatar/parameters/");
                if self.prefix != prefix {
                    self.prefix = prefix.into();
                    self.invalidate_routes();
                }
            }
            "send_all" => {
                let send_all = vf_sdk::json_bool(value, true);
                if self.send_all != send_all {
                    self.send_all = send_all;
                    self.invalidate_routes();
                    if let Some(q) = &self.query {
                        q.set_send_all(self.send_all);
                    }
                }
            }
            "oscquery" => {
                let on = vf_sdk::json_bool(value, true);
                if on != self.oscquery {
                    self.oscquery = on;
                    self.invalidate_routes();
                    if self.sock.is_some() || self.query.is_some() {
                        self.restart_query();
                    }
                }
            }
            "binary_bits" => {
                let bits = vf_sdk::json_i64(value, 0).clamp(0, 8);
                if self.binary_bits != bits {
                    self.binary_bits = bits;
                    self.invalidate_routes();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        if let Some(e) = &self.last_error {
            return NodeStatus::warn(e.clone());
        }
        if let Some(q) = &self.query {
            NodeStatus::ok(format!("{}  sent {}", q.snapshot().status, self.sent))
        } else {
            NodeStatus::ok(format!(
                "{}:{}  sent {}",
                self.host_str, self.port, self.sent
            ))
        }
    }
}

fn push_native_eye(
    msgs: &mut Vec<OscMessage>,
    frame: &VfUnifiedFrame,
    gaze: bool,
    lid: bool,
    last_gaze: &mut Option<(f32, f32, f32, f32)>,
    last_lid: &mut Option<f32>,
) {
    if gaze {
        let (lp, ly) = gaze_pitch_yaw(frame.eye.left.gaze);
        let (rp, ry) = gaze_pitch_yaw(frame.eye.right.gaze);
        let cur = (lp, ly, rp, ry);
        let changed = match *last_gaze {
            Some(prev) => {
                (prev.0 - cur.0).abs()
                    + (prev.1 - cur.1).abs()
                    + (prev.2 - cur.2).abs()
                    + (prev.3 - cur.3).abs()
                    >= 1e-3
            }
            None => true,
        };
        if changed {
            *last_gaze = Some(cur);
            msgs.push(OscMessage {
                addr: "/tracking/eye/LeftRightPitchYaw".into(),
                args: vec![
                    OscType::Float(lp),
                    OscType::Float(ly),
                    OscType::Float(rp),
                    OscType::Float(ry),
                ],
            });
        }
    }
    if lid {
        let closed = 1.0 - frame.eye.combined_openness();
        let changed = match *last_lid {
            Some(prev) => (prev - closed).abs() >= 1e-3,
            None => true,
        };
        if changed {
            *last_lid = Some(closed);
            msgs.push(OscMessage {
                addr: "/tracking/eye/EyesClosedAmount".into(),
                args: vec![OscType::Float(closed)],
            });
        }
    }
}

/// Encode a single float message — used by tests.
pub fn encode_float_message(addr: &str, value: f32) -> Result<Vec<u8>> {
    let pkt = OscPacket::Message(OscMessage {
        addr: addr.into(),
        args: vec![OscType::Float(value)],
    });
    encoder::encode(&pkt).map_err(|e| vf_sdk::SdkError::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oscquery::test_snapshot;
    use rosc::decoder;
    use vf_abi::UnifiedExpression as U;

    fn output() -> (VrcOscOutput, UdpSocket) {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_nonblocking(true).unwrap();
        let host = unsafe { Host::from_raw(std::ptr::null(), 0) };
        let node = VrcOscOutput::create(
            host,
            &serde_json::json!({
                "oscquery": false, "port": receiver.local_addr().unwrap().port(),
            }),
        )
        .unwrap();
        (node, receiver)
    }

    fn drain(receiver: &UdpSocket) -> Vec<OscMessage> {
        fn flatten(packet: OscPacket, messages: &mut Vec<OscMessage>) {
            match packet {
                OscPacket::Message(m) => messages.push(m),
                OscPacket::Bundle(b) => {
                    for p in b.content {
                        flatten(p, messages);
                    }
                }
            }
        }
        let mut messages = Vec::new();
        let mut bytes = [0u8; crate::packet::MAX_DATAGRAM_BYTES];
        loop {
            match receiver.recv(&mut bytes) {
                Ok(n) => {
                    let (rest, packet) = decoder::decode_udp(&bytes[..n]).unwrap();
                    assert!(rest.is_empty());
                    flatten(packet, &mut messages);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("receive: {e}"),
            }
        }
        messages
    }

    fn tick(
        node: &mut VrcOscOutput,
        receiver: &UdpSocket,
        frame: &VfUnifiedFrame,
        snap: Option<&QuerySnapshot>,
        now_us: u64,
    ) -> Vec<OscMessage> {
        node.process_frame(
            &ProcessCtx {
                tick: now_us / 10000,
                dt_us: 10000,
                now_us,
            },
            frame,
            snap,
        )
        .unwrap();
        drain(receiver)
    }

    fn contains(messages: &[OscMessage], addr: &str, value: OscType) -> bool {
        messages
            .iter()
            .any(|m| m.addr == addr && m.args == [value.clone()])
    }

    #[test]
    fn binary_delta_uses_wire_value_even_below_float_epsilon() {
        let (mut node, receiver) = output();
        let mut snap = test_snapshot(&[
            "/avatar/parameters/FT/v2/JawOpen",
            "/avatar/parameters/FT/v2/JawOpen1",
            "/avatar/parameters/FT/v2/JawOpen2",
            "/avatar/parameters/FT/v2/JawOpen4",
            "/avatar/parameters/FT/v2/JawOpen8",
            "/avatar/parameters/FT/v2/JawOpen16",
        ]);
        snap.send_port = node.port;
        let mut frame = VfUnifiedFrame::default();
        frame.shapes[U::JawOpen.index()] = 0.35;
        let first = tick(&mut node, &receiver, &frame, Some(&snap), 0);
        assert_eq!(first.len(), 6); // auto bits, despite binary_bits=0
        frame.shapes[U::JawOpen.index()] = 0.351;
        let jitter = tick(&mut node, &receiver, &frame, Some(&snap), 10000);
        assert_eq!(jitter.len(), 1);
        assert!(contains(
            &jitter,
            "/avatar/parameters/FT/v2/JawOpen",
            OscType::Float(0.351)
        ));

        frame.shapes[U::JawOpen.index()] = 0.49999;
        tick(&mut node, &receiver, &frame, Some(&snap), 20000);
        frame.shapes[U::JawOpen.index()] = 0.50001;
        let boundary = tick(&mut node, &receiver, &frame, Some(&snap), 30000);
        assert_eq!(boundary.len(), 5);
        assert!(contains(
            &boundary,
            "/avatar/parameters/FT/v2/JawOpen16",
            OscType::Bool(true)
        ));
        assert!(
            boundary
                .iter()
                .all(|m| matches!(m.args[0], OscType::Bool(_)))
        );
        assert!(tick(&mut node, &receiver, &frame, Some(&snap), 40000).is_empty());
    }

    #[test]
    fn avatar_loading_and_same_avatar_route_changes_resend_unchanged_values() {
        let (mut node, receiver) = output();
        let mut snap = test_snapshot(&[]);
        snap.send_port = node.port;
        snap.avatar_loaded = false;
        let frame = VfUnifiedFrame::default();
        assert!(!tick(&mut node, &receiver, &frame, Some(&snap), 0).is_empty());
        snap.avatar_loaded = true;
        std::sync::Arc::make_mut(&mut snap.avatar_params)
            .insert("/avatar/parameters/FT/v2/JawOpen".into());
        let loaded = tick(&mut node, &receiver, &frame, Some(&snap), 10000);
        assert_eq!(loaded.len(), 1);
        assert!(contains(
            &loaded,
            "/avatar/parameters/FT/v2/JawOpen",
            OscType::Float(0.0)
        ));
        snap.status = "status-only refresh".into();
        assert!(tick(&mut node, &receiver, &frame, Some(&snap), 20000).is_empty());

        // A previously absent name was negatively cached; a same-ID update
        // must make it routable without waiting for the expression to move.
        std::sync::Arc::make_mut(&mut snap.avatar_params)
            .insert("/avatar/parameters/FT/v2/TongueOut".into());
        let added = tick(&mut node, &receiver, &frame, Some(&snap), 30000);
        assert!(contains(
            &added,
            "/avatar/parameters/FT/v2/TongueOut",
            OscType::Float(0.0)
        ));
        snap.avatar_id = "another-avatar-with-the-same-parameters".into();
        assert_eq!(
            tick(&mut node, &receiver, &frame, Some(&snap), 40000),
            added
        );
    }

    #[test]
    fn filtering_and_native_eye_changes_invalidate_routes() {
        let (mut node, receiver) = output();
        let mut snap = test_snapshot(&["/avatar/parameters/v2/JawOpen"]);
        snap.send_port = node.port;
        let frame = VfUnifiedFrame::default();
        assert_eq!(tick(&mut node, &receiver, &frame, Some(&snap), 0).len(), 1);
        snap.force_all = true;
        assert!(tick(&mut node, &receiver, &frame, Some(&snap), 10000).len() > 200);
        snap.force_all = false;
        assert_eq!(
            tick(&mut node, &receiver, &frame, Some(&snap), 20000).len(),
            1
        );
        snap.send_all = true;
        assert!(tick(&mut node, &receiver, &frame, Some(&snap), 30000).len() > 200);
        snap.send_all = false;
        snap.native_gaze = true;
        snap.native_lid = true;
        let native = tick(&mut node, &receiver, &frame, Some(&snap), 40000);
        assert!(
            native
                .iter()
                .any(|m| m.addr == "/tracking/eye/LeftRightPitchYaw")
        );
        assert!(
            native
                .iter()
                .any(|m| m.addr == "/tracking/eye/EyesClosedAmount")
        );
    }

    #[test]
    fn config_and_destination_changes_resend_current_state() {
        let (mut node, receiver) = output();
        let frame = VfUnifiedFrame::default();
        tick(&mut node, &receiver, &frame, None, 0);
        assert!(tick(&mut node, &receiver, &frame, None, 10000).is_empty());
        node.set_param("prefix", &serde_json::json!("/new/"))
            .unwrap();
        let moved = tick(&mut node, &receiver, &frame, None, 20000);
        assert!(!moved.is_empty());
        assert!(moved.iter().all(|m| m.addr.starts_with("/new/")));
        node.set_param("binary_bits", &serde_json::json!(1))
            .unwrap();
        let binary = tick(&mut node, &receiver, &frame, None, 30000);
        assert!(contains(&binary, "/new/v2/JawOpen1", OscType::Bool(false)));

        let next = UdpSocket::bind("127.0.0.1:0").unwrap();
        next.set_nonblocking(true).unwrap();
        node.set_param(
            "port",
            &serde_json::json!(next.local_addr().unwrap().port()),
        )
        .unwrap();
        assert_eq!(tick(&mut node, &next, &frame, None, 40000), binary);
        assert!(drain(&receiver).is_empty());
    }

    #[test]
    fn periodic_refresh_repairs_lost_deltas_including_native_eye() {
        let (mut node, receiver) = output();
        let mut snap = test_snapshot(&[
            "/avatar/parameters/v2/JawOpen",
            "/avatar/parameters/v2/JawOpen1",
            "/avatar/parameters/EyeTrackingActive",
        ]);
        snap.send_port = node.port;
        snap.native_gaze = true;
        snap.native_lid = true;
        let frame = VfUnifiedFrame::default();
        let initial = tick(&mut node, &receiver, &frame, Some(&snap), 0);
        assert_eq!(initial.len(), 5);
        assert!(
            tick(
                &mut node,
                &receiver,
                &frame,
                Some(&snap),
                FULL_REFRESH_US - 1
            )
            .is_empty()
        );
        assert_eq!(
            tick(&mut node, &receiver, &frame, Some(&snap), FULL_REFRESH_US),
            initial
        );
    }

    #[test]
    fn send_error_is_reported_and_does_not_suppress_retry() {
        let (mut node, receiver) = output();
        let port = node.port;
        // Inject an invalid UDP destination; normal parameter validation
        // prevents this, but transport failures must still remain retryable.
        node.port = 0;
        let frame = VfUnifiedFrame::default();
        let ctx = ProcessCtx {
            tick: 0,
            dt_us: 10000,
            now_us: 0,
        };
        assert!(node.process_frame(&ctx, &frame, None).is_err());
        assert!(node.last_error.is_some());
        assert!(node.last.is_empty());
        assert!(node.last_binary.is_empty());
        assert!(!node.sent_active);
        node.set_param("port", &serde_json::json!(port)).unwrap();
        assert!(tick(&mut node, &receiver, &frame, None, 10000).len() > 200);
        assert!(node.last_error.is_none());
    }

    #[test]
    fn osc_float_roundtrip() {
        let bytes = encode_float_message("/avatar/parameters/v2/JawOpen", 0.5).unwrap();
        let (_, pkt) = decoder::decode_udp(&bytes).unwrap();
        match pkt {
            OscPacket::Message(m) => {
                assert_eq!(m.addr, "/avatar/parameters/v2/JawOpen");
                assert_eq!(m.args[0], OscType::Float(0.5));
            }
            _ => panic!("expected message"),
        }
    }
}
