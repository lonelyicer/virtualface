use crate::packet::parse_pico_packet;
use crate::ue::UeMapper;
use std::net::UdpSocket;
use std::sync::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use vf_abi::VfUnifiedFrame;
use vf_sdk::{
    Category, Host, Node, NodeDescriptor, NodeStatus, ParamDef, PortDesc, PortIo, ProcessCtx,
    Result, param_i64,
};

struct Slot {
    latest: Mutex<Option<(VfUnifiedFrame, Instant)>>,
    timeouts: std::sync::atomic::AtomicU64,
}

pub struct PicoUdpSource {
    host: Host,
    port: u16,
    slot: Arc<Slot>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Node for PicoUdpSource {
    fn descriptor() -> NodeDescriptor {
        NodeDescriptor::new("pico.udp_source", "PICO UDP Source", Category::Input)
            .source()
            .output(PortDesc::unified("unified"))
            .output(PortDesc::float("timeout"))
            .param(ParamDef::int("port", "UDP Port", 29765, 1, 65535))
    }

    fn create(host: Host, config: &serde_json::Value) -> Result<Self> {
        let port = param_i64(config, "port", 29765).clamp(1, 65535) as u16;
        Ok(Self {
            host,
            port,
            slot: Arc::new(Slot {
                latest: Mutex::new(None),
                timeouts: std::sync::atomic::AtomicU64::new(0),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        })
    }

    fn start(&mut self) -> Result<()> {
        self.stop.store(false, Ordering::SeqCst);
        let stop = self.stop.clone();
        let slot = self.slot.clone();
        let host = self.host;
        let port = self.port;
        self.thread = Some(thread::spawn(move || recv_loop(port, host, slot, stop)));
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }

    fn process(&mut self, ctx: &ProcessCtx, io: &mut PortIo<'_>) -> Result<()> {
        let (mut frame, timeout) = {
            let g = self.slot.latest.lock().unwrap();
            match &*g {
                Some((f, t)) => {
                    let age = t.elapsed().as_secs_f32();
                    (*f, age)
                }
                None => (VfUnifiedFrame::default(), 1.0),
            }
        };
        frame.timestamp_us = ctx.now_us;
        *io.output_unified_mut(0)? = frame;
        io.output_float(1, timeout)?;
        Ok(())
    }

    fn set_param(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        if key == "port" {
            let p = vf_sdk::json_i64(value, self.port as i64).clamp(1, 65535) as u16;
            if p != self.port {
                self.port = p;
                let running = self.thread.is_some();
                if running {
                    self.stop();
                    self.start()?;
                }
            }
        }
        Ok(())
    }

    fn status(&self) -> NodeStatus {
        let age = self
            .slot
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .map(|(_, t)| t.elapsed());
        match age {
            Some(d) if d < Duration::from_millis(200) => {
                NodeStatus::ok(format!("udp :{}  live", self.port))
            }
            Some(d) => {
                NodeStatus::warn(format!("udp :{}  stale {:.1}s", self.port, d.as_secs_f32()))
            }
            None => NodeStatus::idle(format!("udp :{}  waiting", self.port)),
        }
    }
}

fn recv_loop(port: u16, host: Host, slot: Arc<Slot>, stop: Arc<AtomicBool>) {
    let sock = match UdpSocket::bind(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(e) => {
            host.error(&format!("bind :{port} failed: {e}"));
            return;
        }
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(100)));
    host.info(&format!("listening on UDP {port}"));
    let mut buf = [0u8; 2048];
    let mut timeout_streak = 0u32;
    let mut mapper = UeMapper::default();
    while !stop.load(Ordering::Relaxed) {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                timeout_streak = 0;
                if let Some(frame) = parse_pico_packet(&buf[..n]) {
                    let unified = mapper.map_pico(&frame.weights, host.now_us());
                    *slot.latest.lock().unwrap() = Some((unified, Instant::now()));
                    host.wake();
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                timeout_streak += 1;
                if timeout_streak > 600 {
                    host.warn("PICO UDP timeout (no packets for ~60s)");
                    slot.timeouts.fetch_add(1, Ordering::Relaxed);
                    timeout_streak = 0;
                }
            }
            Err(e) => {
                if !stop.load(Ordering::Relaxed) {
                    host.warn(&format!("recv error: {e}"));
                }
            }
        }
    }
}
