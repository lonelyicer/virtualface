use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::net::{SocketAddr, UdpSocket};
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_i64, param_string,
};

pub struct OscRawOutput {
    host_str: String,
    port: u16,
    address: String,
    sock: Option<UdpSocket>,
    addr: Option<SocketAddr>,
}

impl Node for OscRawOutput {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("vrc.osc_raw", "OSC Raw Float", Category::Output)
            .input(PortDesc::float("value"))
            .param(ParamDef::string("host", "Host", "127.0.0.1"))
            .param(ParamDef::int("port", "Port", 9000, 1, 65535))
            .param(ParamDef::string(
                "address",
                "OSC Address",
                "/avatar/parameters/debug",
            ))
    }

    fn create(_host: Host, config: &serde_json::Value) -> Result<Self> {
        Ok(Self {
            host_str: param_string(config, "host", "127.0.0.1"),
            port: param_i64(config, "port", 9000) as u16,
            address: param_string(config, "address", "/avatar/parameters/debug"),
            sock: None,
            addr: None,
        })
    }

    fn process(&mut self, _ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let Some(v) = io.input_float(0) else {
            return Ok(());
        };
        if self.sock.is_none() {
            let sock = UdpSocket::bind("0.0.0.0:0")?;
            sock.set_nonblocking(true)?;
            let addr: SocketAddr = format!("{}:{}", self.host_str, self.port)
                .parse()
                .map_err(|e| vf_sdk::SdkError::other(format!("{e}")))?;
            self.addr = Some(addr);
            self.sock = Some(sock);
        }
        let pkt = OscPacket::Message(OscMessage {
            addr: self.address.clone(),
            args: vec![OscType::Float(v)],
        });
        if let Ok(bytes) = encoder::encode(&pkt)
            && let (Some(sock), Some(addr)) = (&self.sock, self.addr)
        {
            let _ = sock.send_to(&bytes, addr);
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
                self.port = vf_sdk::json_i64(value, 9000) as u16;
                self.sock = None;
            }
            "address" => self.address = value.as_str().unwrap_or("/debug").into(),
            _ => {}
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        NodeStatus::ok(format!(
            "{} → {}:{}",
            self.address, self.host_str, self.port
        ))
    }
}
