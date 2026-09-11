use crate::oscquery::{OscQueryHub, gaze_pitch_yaw};
use crate::params::{derive_v2, encode_binary};
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType, encoder};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use vf_abi::VfUnifiedFrame;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_bool, param_i64, param_string,
};

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
    last: HashMap<String, f32>,
    last_gaze: Option<(f32, f32, f32, f32)>,
    last_lid: Option<f32>,
    seen_avatar: String,
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
        Ok(())
    }

    fn send_packet(&mut self, pkt: OscPacket) {
        let Ok(bytes) = encoder::encode(&pkt) else {
            return;
        };
        if let (Some(sock), Some(addr)) = (&self.sock, self.addr) {
            match sock.send_to(&bytes, addr) {
                Ok(_) => self.sent += 1,
                Err(e) => self.last_error = Some(e.to_string()),
            }
        }
    }

    fn send_msgs(&mut self, msgs: Vec<OscMessage>) {
        if msgs.is_empty() {
            return;
        }
        if msgs.len() == 1 {
            self.send_packet(OscPacket::Message(msgs.into_iter().next().unwrap()));
            return;
        }
        self.send_packet(OscPacket::Bundle(OscBundle {
            timetag: OscTime {
                seconds: 0,
                fractional: 1,
            },
            content: msgs.into_iter().map(OscPacket::Message).collect(),
        }));
    }

    fn restart_query(&mut self) {
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
                "Binary Bits (0=float)",
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
            binary_bits: param_i64(config, "binary_bits", 0),
            oscquery: param_bool(config, "oscquery", true),
            query: None,
            sock: None,
            addr: None,
            last: HashMap::new(),
            last_gaze: None,
            last_lid: None,
            seen_avatar: String::new(),
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
        self.sent_active = false;
        self.last.clear();
        self.last_gaze = None;
        self.last_lid = None;
        self.seen_avatar.clear();
        Ok(())
    }

    fn stop(&mut self) {
        self.query = None;
        self.sock = None;
        self.addr = None;
        self.last.clear();
        self.last_gaze = None;
        self.last_lid = None;
        self.seen_avatar.clear();
        self.sent_active = false;
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let frame = io.input_unified(0)?;
        let snap = self.query.as_ref().map(|q| q.snapshot());
        let host = snap
            .as_ref()
            .map(|s| s.send_host.clone())
            .unwrap_or_else(|| self.host_str.clone());
        let port = snap.as_ref().map(|s| s.send_port).unwrap_or(self.port);
        let native_gaze = snap.as_ref().map(|s| s.native_gaze).unwrap_or(false);
        let native_lid = snap.as_ref().map(|s| s.native_lid).unwrap_or(false);
        if let Some(s) = &snap
            && s.avatar_id != self.seen_avatar
        {
            self.seen_avatar = s.avatar_id.clone();
            self.last.clear();
            self.last_gaze = None;
            self.last_lid = None;
            self.sent_active = false;
        }
        self.ensure_sock(&host, port)?;

        let params = derive_v2(frame);
        let mut msgs: Vec<OscMessage> = Vec::new();
        let prefix = self.prefix.clone();

        if !self.sent_active {
            for name in [
                "EyeTrackingActive",
                "ExpressionTrackingActive",
                "LipTrackingActive",
            ] {
                if let Some(addr) = match &snap {
                    Some(s) => s.target_addr(name, &prefix),
                    None => Some(format!("{prefix}{name}")),
                } {
                    msgs.push(OscMessage {
                        addr,
                        args: vec![OscType::Bool(true)],
                    });
                }
            }
            self.sent_active = true;
        }

        for (name, value) in params {
            let Some(addr) = (match &snap {
                Some(s) => s.target_addr(&name, &prefix),
                None => Some(format!("{prefix}{name}")),
            }) else {
                continue;
            };
            if let Some(prev) = self.last.get(&name)
                && (prev - value).abs() < 1e-4
            {
                continue;
            }
            self.last.insert(name.clone(), value);
            let bits = if self.binary_bits > 0 {
                self.binary_bits as u32
            } else {
                snap.as_ref().map(|s| s.auto_bits(&addr)).unwrap_or(0)
            };
            if bits > 0 {
                for (bn, bv) in encode_binary(&addr, value, bits) {
                    if snap.as_ref().map(|s| s.has_address(&bn)).unwrap_or(true) {
                        msgs.push(OscMessage {
                            addr: bn,
                            args: vec![OscType::Bool(bv)],
                        });
                    }
                }
            }
            msgs.push(OscMessage {
                addr,
                args: vec![OscType::Float(value)],
            });
        }

        if native_gaze || native_lid {
            push_native_eye(
                &mut msgs,
                frame,
                native_gaze,
                native_lid,
                &mut self.last_gaze,
                &mut self.last_lid,
            );
        }

        self.send_msgs(msgs);
        Ok(())
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
            "prefix" => self.prefix = value.as_str().unwrap_or("/avatar/parameters/").into(),
            "send_all" => {
                self.send_all = vf_sdk::json_bool(value, true);
                if let Some(q) = &self.query {
                    q.set_send_all(self.send_all);
                }
            }
            "oscquery" => {
                let on = vf_sdk::json_bool(value, true);
                if on != self.oscquery {
                    self.oscquery = on;
                    if self.sock.is_some() || self.query.is_some() {
                        self.restart_query();
                    }
                }
            }
            "binary_bits" => self.binary_bits = vf_sdk::json_i64(value, 0),
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
    use rosc::decoder;

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
