//! OSCQuery advertise/discover + avatar parameter filtering (VRCFT-style).

use crate::avatar_cfg::find_param_address;
use rosc::{OscPacket, OscType, decoder};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use vf_sdk::Host;

const VRC_QUERY_PORT: u16 = 9001;

#[derive(Clone, Debug)]
pub struct QuerySnapshot {
    pub send_host: String,
    pub send_port: u16,
    pub send_all: bool,
    pub force_all: bool,
    pub avatar_id: String,
    pub avatar_params: HashSet<String>,
    pub native_gaze: bool,
    pub native_lid: bool,
    pub discovered: bool,
    pub avatar_loaded: bool,
    pub query_host: String,
    pub query_port: u16,
    pub http_port: u16,
    pub osc_in_port: u16,
    pub status: String,
}

impl QuerySnapshot {
    pub fn target_addr(&self, name: &str, prefix: &str) -> Option<String> {
        if let Some(addr) = find_param_address(&self.avatar_params, name) {
            return Some(addr);
        }
        if self.send_all || self.force_all || !self.avatar_loaded {
            Some(format!("{prefix}{name}"))
        } else {
            None
        }
    }

    pub fn has_address(&self, addr: &str) -> bool {
        !self.avatar_loaded || self.send_all || self.force_all || self.avatar_params.contains(addr)
    }

    pub fn auto_bits(&self, addr: &str) -> u32 {
        let mut n = 0u32;
        for i in 0..8 {
            let idx = 1u32 << i;
            if self.avatar_params.contains(&format!("{addr}{idx}")) {
                n = i + 1;
            } else {
                break;
            }
        }
        n
    }
}

pub struct OscQueryHub {
    state: Arc<Mutex<HubState>>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

pub(crate) struct HubState {
    snap: QuerySnapshot,
    avatar_dirty: bool,
    host_info_hit: bool,
}

impl OscQueryHub {
    pub fn start(
        host: Host,
        send_all: bool,
        manual_host: String,
        manual_port: u16,
    ) -> vf_sdk::Result<Self> {
        let suffix = (host.now_us() % 0xffff) as u16;
        let service_name = format!("VirtualFace-{suffix:04x}");
        let osc_sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, VRC_QUERY_PORT))
            .or_else(|_| UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)))
            .or_else(|_| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))?;
        osc_sock.set_read_timeout(Some(Duration::from_millis(150)))?;
        let osc_in_port = osc_sock.local_addr()?.port();

        let http = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        http.set_nonblocking(true)?;
        let http_port = http.local_addr()?.port();

        let snap = QuerySnapshot {
            send_host: manual_host.clone(),
            send_port: manual_port,
            send_all,
            force_all: false,
            avatar_id: String::new(),
            avatar_params: HashSet::new(),
            native_gaze: false,
            native_lid: false,
            discovered: false,
            avatar_loaded: false,
            query_host: String::new(),
            query_port: 0,
            http_port,
            osc_in_port,
            status: format!("oscquery http=:{http_port} osc_in=:{osc_in_port} waiting"),
        };
        let state = Arc::new(Mutex::new(HubState {
            snap,
            avatar_dirty: true,
            host_info_hit: false,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();

        {
            let stop = stop.clone();
            let state = state.clone();
            let name = service_name.clone();
            threads.push(thread::spawn(move || {
                http_loop(http, stop, state, name, osc_in_port);
            }));
        }
        {
            let stop = stop.clone();
            let state = state.clone();
            threads.push(thread::spawn(move || osc_recv_loop(osc_sock, stop, state)));
        }
        {
            let stop = stop.clone();
            let state = state.clone();
            let host = host;
            threads.push(thread::spawn(move || {
                probe_loop(stop, state, manual_host, manual_port, host);
            }));
        }

        threads.push(crate::mdns_vrc::spawn(
            stop.clone(),
            state.clone(),
            host,
            service_name,
            http_port,
            osc_in_port,
        ));

        Ok(Self {
            state,
            stop,
            threads,
        })
    }

    pub fn snapshot(&self) -> QuerySnapshot {
        self.state.lock().unwrap().snap.clone()
    }

    pub fn set_send_all(&self, send_all: bool) {
        let mut g = self.state.lock().unwrap();
        g.snap.send_all = send_all;
        g.avatar_dirty = true;
    }

    pub fn set_manual_target(&self, host: String, port: u16) {
        let mut g = self.state.lock().unwrap();
        if !g.snap.discovered {
            g.snap.send_host = host;
            g.snap.send_port = port;
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for h in self.threads.drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for OscQueryHub {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn apply_query_endpoint(state: &Arc<Mutex<HubState>>, host: String, port: u16) {
    {
        let g = state.lock().unwrap();
        if g.snap.discovered && g.snap.query_host == host && g.snap.query_port == port {
            return;
        }
    }
    if let Some((osc_ip, osc_port)) = fetch_host_info(&host, port) {
        let mut g = state.lock().unwrap();
        g.snap.discovered = true;
        g.snap.query_host = host.clone();
        g.snap.query_port = port;
        g.snap.send_host = osc_ip;
        g.snap.send_port = osc_port;
        g.snap.status = format!("oscquery {host}:{port} → {}:{}", g.snap.send_host, osc_port);
        g.avatar_dirty = true;
    } else {
        let mut g = state.lock().unwrap();
        g.snap.discovered = true;
        g.snap.query_host = host.clone();
        g.snap.query_port = port;
        g.snap.status = format!("oscquery found {host}:{port}");
        g.avatar_dirty = true;
    }
}

fn probe_loop(
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<HubState>>,
    manual_host: String,
    _manual_port: u16,
    host: Host,
) {
    let mut last_probe = Instant::now() - Duration::from_secs(5);
    let mut last_avatar = Instant::now() - Duration::from_secs(5);
    let mut logged_id = String::new();
    while !stop.load(Ordering::Relaxed) {
        let (dirty, discovered) = {
            let mut g = state.lock().unwrap();
            let hit = std::mem::take(&mut g.host_info_hit);
            let dirty = std::mem::take(&mut g.avatar_dirty) || hit;
            (dirty, g.snap.discovered)
        };
        if !discovered && last_probe.elapsed() > Duration::from_secs(2) {
            last_probe = Instant::now();
            if fetch_host_info("127.0.0.1", VRC_QUERY_PORT).is_some() {
                apply_query_endpoint(&state, "127.0.0.1".into(), VRC_QUERY_PORT);
            } else if manual_host != "127.0.0.1"
                && fetch_host_info(&manual_host, VRC_QUERY_PORT).is_some()
            {
                apply_query_endpoint(&state, manual_host.clone(), VRC_QUERY_PORT);
            }
        }
        if dirty || last_avatar.elapsed() > Duration::from_secs(2) {
            last_avatar = Instant::now();
            refresh_avatar(&state, &host, &mut logged_id);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn refresh_avatar(state: &Arc<Mutex<HubState>>, host: &Host, logged_id: &mut String) {
    let pending_id = state.lock().unwrap().snap.avatar_id.clone();
    let (qhost, qport, discovered) = {
        let g = state.lock().unwrap();
        (
            g.snap.query_host.clone(),
            g.snap.query_port,
            g.snap.discovered,
        )
    };
    let mut query_id = String::new();
    let mut query_params = HashSet::new();
    let qh = if discovered && !qhost.is_empty() && qport != 0 {
        (qhost.as_str(), qport)
    } else {
        ("127.0.0.1", VRC_QUERY_PORT)
    };
    if let Some(body) = http_get(qh.0, qh.1, "/avatar") {
        if let Ok(value) = serde_json::from_str::<Value>(&body) {
            query_id = avatar_id(&value);
            query_params = collect_typed_paths(&value);
        }
    }
    let want_id = if !pending_id.is_empty() {
        pending_id
    } else {
        query_id.clone()
    };
    let file = if want_id.is_empty() {
        crate::avatar_cfg::latest_avatar_config()
    } else {
        crate::avatar_cfg::find_avatar_config(&want_id)
    };
    if let Some(cfg) = file {
        if cfg.id != *logged_id {
            *logged_id = cfg.id.clone();
            let label = if cfg.name.is_empty() {
                cfg.id.clone()
            } else {
                cfg.name.clone()
            };
            host.info(&format!(
                "avatar config {label}  {} params",
                cfg.addresses.len()
            ));
        }
        commit_avatar(state, cfg.id, cfg.addresses, &cfg.name);
        return;
    }
    if !query_params.is_empty() || !query_id.is_empty() {
        commit_avatar(state, query_id, query_params, "");
    }
}

fn commit_avatar(
    state: &Arc<Mutex<HubState>>,
    id: String,
    params: HashSet<String>,
    avatar_name: &str,
) {
    let native_gaze = needs_native_gaze(&params);
    let native_lid = needs_native_lid(&params);
    let mut g = state.lock().unwrap();
    if !id.is_empty() {
        g.snap.avatar_id = id.clone();
    }
    g.snap.avatar_params = params;
    g.snap.native_gaze = native_gaze;
    g.snap.native_lid = native_lid;
    g.snap.avatar_loaded = true;
    let n = g.snap.avatar_params.len();
    let dest = format!("{}:{}", g.snap.send_host, g.snap.send_port);
    let label = if !avatar_name.is_empty() {
        avatar_name.to_string()
    } else if !id.is_empty() {
        id
    } else {
        g.snap.avatar_id.clone()
    };
    g.snap.status = if label.is_empty() {
        format!(
            "waiting  {dest}  http=:{} in=:{}",
            g.snap.http_port, g.snap.osc_in_port
        )
    } else {
        format!("{label}  {n} params → {dest}  http=:{}", g.snap.http_port)
    };
}

fn osc_recv_loop(sock: UdpSocket, stop: Arc<AtomicBool>, state: Arc<Mutex<HubState>>) {
    let mut buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                if let Ok((_, pkt)) = decoder::decode_udp(&buf[..n]) {
                    handle_osc(&pkt, &state);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle_osc(pkt: &OscPacket, state: &Arc<Mutex<HubState>>) {
    match pkt {
        OscPacket::Message(m) => match m.addr.as_str() {
            "/avatar/change" => {
                if let Some(OscType::String(id)) = m.args.first() {
                    let mut g = state.lock().unwrap();
                    g.snap.avatar_id = id.clone();
                    g.avatar_dirty = true;
                } else {
                    state.lock().unwrap().avatar_dirty = true;
                }
            }
            "/vrcft/settings/forceRelevant" => {
                if let Some(OscType::Bool(v)) = m.args.first() {
                    state.lock().unwrap().snap.force_all = *v;
                }
            }
            _ => {}
        },
        OscPacket::Bundle(b) => {
            for c in &b.content {
                handle_osc(c, state);
            }
        }
    }
}

fn http_loop(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<HubState>>,
    name: String,
    osc_port: u16,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_http(stream, &state, &name, osc_port);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_http(
    mut stream: TcpStream,
    state: &Arc<Mutex<HubState>>,
    name: &str,
    osc_port: u16,
) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf)?;
    let req = std::str::from_utf8(&buf[..n]).unwrap_or("");
    let first = req.lines().next().unwrap_or("");
    let method = first.split_whitespace().next().unwrap_or("GET");
    let path = first.split_whitespace().nth(1).unwrap_or("/");
    if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nConnection: close\r\n\r\n";
        stream.write_all(resp.as_bytes())?;
        return Ok(());
    }
    let body = if path.contains("HOST_INFO") {
        state.lock().unwrap().host_info_hit = true;
        host_info_json(name, osc_port)
    } else {
        root_tree_json()
    };
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nPragma: no-cache\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    Ok(())
}

pub fn host_info_json(name: &str, osc_port: u16) -> String {
    json!({
        "NAME": name,
        "OSC_IP": "127.0.0.1",
        "OSC_PORT": osc_port,
        "OSC_TRANSPORT": "UDP",
        "EXTENSIONS": {
            "ACCESS": true,
            "CLIPMODE": false,
            "RANGE": true,
            "TYPE": true,
            "VALUE": true
        }
    })
    .to_string()
}

pub fn root_tree_json() -> String {
    json!({
        "FULL_PATH": "/",
        "ACCESS": 0,
        "CONTENTS": {
            "avatar": {
                "FULL_PATH": "/avatar",
                "ACCESS": 0,
                "CONTENTS": {
                    "change": {
                        "FULL_PATH": "/avatar/change",
                        "ACCESS": 2,
                        "TYPE": "s"
                    }
                }
            },
            "vrcft": {
                "FULL_PATH": "/vrcft",
                "ACCESS": 0,
                "CONTENTS": {
                    "settings": {
                        "FULL_PATH": "/vrcft/settings",
                        "ACCESS": 0,
                        "CONTENTS": {
                            "forceRelevant": {
                                "FULL_PATH": "/vrcft/settings/forceRelevant",
                                "ACCESS": 2,
                                "TYPE": "T"
                            }
                        }
                    }
                }
            }
        }
    })
    .to_string()
}

fn fetch_host_info(host: &str, port: u16) -> Option<(String, u16)> {
    let body = http_get(host, port, "/?HOST_INFO")?;
    let v: Value = serde_json::from_str(&body).ok()?;
    let ip = v
        .get("OSC_IP")
        .and_then(Value::as_str)
        .unwrap_or(host)
        .to_string();
    let osc_port = v.get("OSC_PORT").and_then(Value::as_u64)? as u16;
    Some((ip, osc_port))
}

fn http_get(host: &str, port: u16, path: &str) -> Option<String> {
    let addr = SocketAddr::new(host.parse().ok()?, port);
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(350)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(600)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(400)))
        .ok()?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let idx = text.find("\r\n\r\n")?;
    let (headers, body) = text.split_at(idx + 4);
    if !headers.contains("200") {
        return None;
    }
    Some(body.trim_end_matches('\0').to_string())
}

pub fn collect_typed_paths(node: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_typed_paths_inner(node, &mut out);
    out
}

fn collect_typed_paths_inner(node: &Value, out: &mut HashSet<String>) {
    if let Some(ty) = node.get("TYPE").and_then(Value::as_str) {
        if !ty.is_empty() {
            if let Some(path) = node.get("FULL_PATH").and_then(Value::as_str) {
                out.insert(path.to_string());
            }
        }
    }
    if let Some(map) = node.get("CONTENTS").and_then(Value::as_object) {
        for child in map.values() {
            collect_typed_paths_inner(child, out);
        }
    }
}

pub fn avatar_id(node: &Value) -> String {
    node.get("CONTENTS")
        .and_then(|c| c.get("change"))
        .and_then(|c| c.get("VALUE"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn param_leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn needs_native_gaze(params: &HashSet<String>) -> bool {
    if params.is_empty() {
        return false;
    }
    !params.iter().any(|p| {
        let n = param_leaf(p);
        n.contains("Eye") && (n.contains('X') || n.contains('Y'))
    })
}

pub fn needs_native_lid(params: &HashSet<String>) -> bool {
    if params.is_empty() {
        return false;
    }
    !params.iter().any(|p| {
        let n = param_leaf(p);
        n.contains("Eye") && (n.contains("Open") || n.contains("Lid"))
    })
}

pub fn gaze_pitch_yaw(gaze: [f32; 2]) -> (f32, f32) {
    let yaw = gaze[0].atan().to_degrees();
    let pitch = -gaze[1].atan().to_degrees();
    (pitch, yaw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_info_has_port() {
        let s = host_info_json("VirtualFace-test", 12345);
        assert!(s.contains("\"OSC_PORT\":12345"));
        assert!(s.contains("VirtualFace-test"));
    }

    #[test]
    fn tree_advertises_avatar_change() {
        let s = root_tree_json();
        assert!(s.contains("/avatar/change"));
        assert!(s.contains("/vrcft/settings/forceRelevant"));
    }

    #[test]
    fn collect_nested_params() {
        let v = json!({
            "FULL_PATH": "/avatar",
            "CONTENTS": {
                "parameters": {
                    "FULL_PATH": "/avatar/parameters",
                    "CONTENTS": {
                        "v2": {
                            "FULL_PATH": "/avatar/parameters/v2",
                            "CONTENTS": {
                                "JawOpen": {
                                    "FULL_PATH": "/avatar/parameters/v2/JawOpen",
                                    "TYPE": "f"
                                },
                                "EyeLeftX": {
                                    "FULL_PATH": "/avatar/parameters/v2/EyeLeftX",
                                    "TYPE": "f"
                                },
                                "EyeLidLeft": {
                                    "FULL_PATH": "/avatar/parameters/v2/EyeLidLeft",
                                    "TYPE": "f"
                                }
                            }
                        },
                        "EyeTrackingActive": {
                            "FULL_PATH": "/avatar/parameters/EyeTrackingActive",
                            "TYPE": "T"
                        }
                    }
                }
            }
        });
        let p = collect_typed_paths(&v);
        assert!(p.contains("/avatar/parameters/v2/JawOpen"));
        assert!(p.contains("/avatar/parameters/EyeTrackingActive"));
        assert!(find_param_address(&p, "v2/JawOpen").is_some());
        assert!(find_param_address(&p, "v2/TongueOut").is_none());
        assert!(!needs_native_gaze(&p));
        assert!(!needs_native_lid(&p));
    }

    #[test]
    fn native_eye_when_no_v2_gaze() {
        let mut p = HashSet::new();
        p.insert("/avatar/parameters/v2/JawOpen".into());
        assert!(needs_native_gaze(&p));
        assert!(needs_native_lid(&p));
    }

    #[test]
    fn native_lid_only() {
        let mut p = HashSet::new();
        p.insert("/avatar/parameters/v2/EyeLeftX".into());
        assert!(!needs_native_gaze(&p));
        assert!(needs_native_lid(&p));
    }

    #[test]
    fn pitch_yaw_sign() {
        let (pitch, yaw) = gaze_pitch_yaw([1.0, 1.0]);
        assert!(yaw > 0.0);
        assert!(pitch < 0.0);
    }
}
