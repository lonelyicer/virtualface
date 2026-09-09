use crate::params::{derive_v2, encode_binary};
use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_bool, param_i64, param_string,
};

pub struct VrcOscOutput {
    host_str: String,
    port: u16,
    prefix: String,
    send_all: bool,
    binary_bits: i64,
    sock: Option<UdpSocket>,
    addr: Option<SocketAddr>,
    last: HashMap<String, f32>,
    sent_active: bool,
    sent: u64,
    last_error: Option<String>,
}

impl VrcOscOutput {
    fn ensure_sock(&mut self) -> Result<()> {
        if self.sock.is_some() {
            return Ok(());
        }
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.set_nonblocking(true)?;
        let addr: SocketAddr = format!("{}:{}", self.host_str, self.port)
            .parse()
            .map_err(|e| vf_sdk::SdkError::other(format!("bad address: {e}")))?;
        self.addr = Some(addr);
        self.sock = Some(sock);
        Ok(())
    }

    fn send_packet(&mut self, pkt: OscPacket) {
        if self.ensure_sock().is_err() {
            return;
        }
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
            .param(ParamDef::int(
                "binary_bits",
                "Binary Bits (0=float)",
                0,
                0,
                8,
            ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            host_str: param_string(config, "host", "127.0.0.1"),
            port: param_i64(config, "port", 9000).clamp(1, 65535) as u16,
            prefix: param_string(config, "prefix", "/avatar/parameters/"),
            send_all: param_bool(config, "send_all", true),
            binary_bits: param_i64(config, "binary_bits", 0),
            sock: None,
            addr: None,
            last: HashMap::new(),
            sent_active: false,
            sent: 0,
            last_error: None,
        })
    }

    fn start(&mut self) -> Result<()> {
        self.ensure_sock()?;
        self.sent_active = false;
        Ok(())
    }

    fn stop(&mut self) {
        self.sock = None;
        self.addr = None;
        self.last.clear();
        self.sent_active = false;
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let frame = io.input_unified(0)?;
        let params = derive_v2(frame);
        let mut msgs: Vec<OscMessage> = Vec::new();
        let prefix = self.prefix.clone();
        let bits = self.binary_bits;
        let send_all = self.send_all;

        if !self.sent_active {
            for name in [
                "EyeTrackingActive",
                "ExpressionTrackingActive",
                "LipTrackingActive",
            ] {
                msgs.push(OscMessage {
                    addr: format!("{prefix}{name}"),
                    args: vec![OscType::Bool(true)],
                });
            }
            self.sent_active = true;
        }

        for (name, value) in params {
            if !send_all {
                if let Some(prev) = self.last.get(&name) {
                    if (prev - value).abs() < 1e-4 {
                        continue;
                    }
                }
            } else if let Some(prev) = self.last.get(&name) {
                if (prev - value).abs() < 1e-4 {
                    continue;
                }
            }
            self.last.insert(name.clone(), value);
            if bits > 0 {
                for (bn, bv) in encode_binary(&name, value, bits as u32) {
                    msgs.push(OscMessage {
                        addr: format!("{prefix}{bn}"),
                        args: vec![OscType::Bool(bv)],
                    });
                }
            }
            msgs.push(OscMessage {
                addr: format!("{prefix}{name}"),
                args: vec![OscType::Float(value)],
            });
        }

        if !msgs.is_empty() {
            for m in msgs {
                self.send_packet(OscPacket::Message(m));
            }
        }
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        match key {
            "host" => {
                self.host_str = value.as_str().unwrap_or("127.0.0.1").into();
                self.sock = None;
            }
            "port" => {
                self.port = vf_sdk::json_i64(value, 9000).clamp(1, 65535) as u16;
                self.sock = None;
            }
            "prefix" => self.prefix = value.as_str().unwrap_or("/avatar/parameters/").into(),
            "send_all" => self.send_all = vf_sdk::json_bool(value, true),
            "binary_bits" => self.binary_bits = vf_sdk::json_i64(value, 0),
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        if let Some(e) = &self.last_error {
            NodeStatus::warn(e.clone())
        } else {
            NodeStatus::ok(format!(
                "{}:{}  sent {}",
                self.host_str, self.port, self.sent
            ))
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
