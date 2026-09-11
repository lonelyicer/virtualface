//! VRChat-compatible mDNS: PTR must live in the Answers section.
//!
//! Standard libraries (including mdns-sd) often put PTR in Additional records.
//! VRChat only looks at `answers[0]` as PTR, which is why the in-game OSCQuery
//! popup never appeared.

use crate::oscquery::{HubState, apply_query_endpoint};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use vf_sdk::Host;

const MDNS_PORT: u16 = 5353;
const MDNS_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const TTL: u32 = 120;

pub fn spawn(
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<HubState>>,
    host: Host,
    instance: String,
    http_port: u16,
    osc_port: u16,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let Ok(sock) = bind_mdns() else {
            host.warn("OSCQuery mDNS bind :5353 failed (in-game popup needs this)");
            return;
        };
        host.info(&format!(
            "OSCQuery mDNS {instance} json=:{http_port} osc=:{osc_port}"
        ));
        let dest = SocketAddr::from((MDNS_GROUP, MDNS_PORT));
        let mut last_announce = Instant::now() - Duration::from_secs(2);
        let mut buf = [0u8; 2048];
        while !stop.load(Ordering::Relaxed) {
            if last_announce.elapsed() > Duration::from_secs(1) {
                last_announce = Instant::now();
                let _ = sock.send_to(
                    &advertise_packet(&instance, "_oscjson", "_tcp", http_port),
                    dest,
                );
                let _ = sock.send_to(&advertise_packet(&instance, "_osc", "_udp", osc_port), dest);
            }
            match sock.recv_from(&mut buf) {
                Ok((n, from)) => handle_packet(
                    &buf[..n],
                    from,
                    &sock,
                    &instance,
                    http_port,
                    osc_port,
                    &state,
                ),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(40));
                }
                Err(_) => {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(80));
                }
            }
        }
    })
}

fn bind_mdns() -> std::io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MDNS_PORT).into())?;
    let _ = socket.set_multicast_loop_v4(true);
    socket.join_multicast_v4(&MDNS_GROUP, &Ipv4Addr::UNSPECIFIED)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

fn handle_packet(
    buf: &[u8],
    from: SocketAddr,
    sock: &UdpSocket,
    instance: &str,
    http_port: u16,
    osc_port: u16,
    state: &Arc<Mutex<HubState>>,
) {
    if buf.len() < 12 {
        return;
    }
    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    let qr = flags & 0x8000 != 0;
    let qd = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let an = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    let mut pos = 12usize;
    if qr {
        parse_vrchat_answer(buf, an, &mut pos, from, state);
        return;
    }
    for _ in 0..qd {
        let Some(labels) = read_name(buf, &mut pos) else {
            return;
        };
        if pos + 4 > buf.len() {
            return;
        }
        let qtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        pos += 4;
        if qtype != 12 && qtype != 255 {
            continue;
        }
        let joined = labels.join(".").to_ascii_lowercase();
        let dest = SocketAddr::from((MDNS_GROUP, MDNS_PORT));
        if joined == "_oscjson._tcp.local" {
            let _ = sock.send_to(
                &advertise_packet(instance, "_oscjson", "_tcp", http_port),
                dest,
            );
        } else if joined == "_osc._udp.local" {
            let _ = sock.send_to(&advertise_packet(instance, "_osc", "_udp", osc_port), dest);
        }
    }
}

fn parse_vrchat_answer(
    buf: &[u8],
    an: usize,
    pos: &mut usize,
    from: SocketAddr,
    state: &Arc<Mutex<HubState>>,
) {
    if an == 0 {
        return;
    }
    let Some(owner) = read_name(buf, pos) else {
        return;
    };
    if *pos + 10 > buf.len() {
        return;
    }
    let rtype = u16::from_be_bytes([buf[*pos], buf[*pos + 1]]);
    *pos += 8; // type, class, ttl
    let rdlen = u16::from_be_bytes([buf[*pos], buf[*pos + 1]]) as usize;
    *pos += 2;
    if rtype != 12 {
        return;
    }
    let Some(ptr) = read_name(buf, pos) else {
        return;
    };
    let _ = rdlen;
    let inst = ptr.first().map(|s| s.as_str()).unwrap_or("");
    if !(inst.starts_with("VRChat-Client") || inst.starts_with("ChilloutVR-GameClient")) {
        return;
    }
    if !owner.join(".").eq_ignore_ascii_case("_oscjson._tcp.local") {
        return;
    }
    let mut scan = *pos;
    let mut port = 9001u16;
    let mut ip = Ipv4Addr::LOCALHOST;
    while scan + 10 < buf.len() {
        let Some(_) = read_name(buf, &mut scan) else {
            break;
        };
        if scan + 10 > buf.len() {
            break;
        }
        let ty = u16::from_be_bytes([buf[scan], buf[scan + 1]]);
        scan += 8;
        let n = u16::from_be_bytes([buf[scan], buf[scan + 1]]) as usize;
        scan += 2;
        if scan + n > buf.len() {
            break;
        }
        if ty == 33 && n >= 6 {
            port = u16::from_be_bytes([buf[scan + 4], buf[scan + 5]]);
        }
        if ty == 1 && n == 4 {
            ip = Ipv4Addr::new(buf[scan], buf[scan + 1], buf[scan + 2], buf[scan + 3]);
        }
        scan += n;
    }
    if ip.is_loopback()
        && let SocketAddr::V4(v4) = from
        && !v4.ip().is_loopback()
    {
        ip = *v4.ip();
    }
    apply_query_endpoint(state, ip.to_string(), port);
}

/// Unsolicited / query response with PTR in **Answers**, SRV/A/TXT in Additional.
pub fn advertise_packet(instance: &str, svc: &str, proto: &str, port: u16) -> Vec<u8> {
    let q = [svc, proto, "local"];
    let qual = [instance, svc, proto, "local"];
    let host = [
        instance,
        svc.trim_start_matches('_'),
        proto.trim_start_matches('_'),
    ];
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(&0u16.to_be_bytes()); // ID
    out.extend_from_slice(&0x8400u16.to_be_bytes()); // QR + AA/CONFLICT
    out.extend_from_slice(&0u16.to_be_bytes()); // QD
    out.extend_from_slice(&1u16.to_be_bytes()); // AN = PTR
    out.extend_from_slice(&0u16.to_be_bytes()); // NS
    out.extend_from_slice(&3u16.to_be_bytes()); // AR = TXT, SRV, A

    write_name(&mut out, &q);
    out.extend_from_slice(&12u16.to_be_bytes()); // PTR
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&TTL.to_be_bytes());
    let ptr_data = name_bytes(&qual);
    out.extend_from_slice(&(ptr_data.len() as u16).to_be_bytes());
    out.extend_from_slice(&ptr_data);

    write_name(&mut out, &qual);
    out.extend_from_slice(&16u16.to_be_bytes()); // TXT
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&TTL.to_be_bytes());
    let txt = txt_bytes("txtvers=1");
    out.extend_from_slice(&(txt.len() as u16).to_be_bytes());
    out.extend_from_slice(&txt);

    write_name(&mut out, &qual);
    out.extend_from_slice(&33u16.to_be_bytes()); // SRV
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&TTL.to_be_bytes());
    let mut srv = Vec::new();
    srv.extend_from_slice(&0u16.to_be_bytes());
    srv.extend_from_slice(&0u16.to_be_bytes());
    srv.extend_from_slice(&port.to_be_bytes());
    write_name(&mut srv, &host);
    out.extend_from_slice(&(srv.len() as u16).to_be_bytes());
    out.extend_from_slice(&srv);

    write_name(&mut out, &host);
    out.extend_from_slice(&1u16.to_be_bytes()); // A
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&TTL.to_be_bytes());
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend_from_slice(&Ipv4Addr::LOCALHOST.octets());
    out
}

fn txt_bytes(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.push(s.len() as u8);
    v.extend_from_slice(s.as_bytes());
    v
}

fn name_bytes(labels: &[&str]) -> Vec<u8> {
    let mut v = Vec::new();
    write_name(&mut v, labels);
    v
}

fn write_name(out: &mut Vec<u8>, labels: &[&str]) {
    for l in labels {
        out.push(l.len() as u8);
        out.extend_from_slice(l.as_bytes());
    }
    out.push(0);
}

fn read_name(msg: &[u8], pos: &mut usize) -> Option<Vec<String>> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_pos = 0;
    let mut hops = 0;
    loop {
        if hops > 16 || *pos >= msg.len() {
            return None;
        }
        let len = msg[*pos];
        if len & 0xC0 == 0xC0 {
            if *pos + 1 >= msg.len() {
                return None;
            }
            let ptr = (((len as usize) & 0x3F) << 8) | msg[*pos + 1] as usize;
            if !jumped {
                return_pos = *pos + 2;
                jumped = true;
            }
            *pos = ptr;
            hops += 1;
            continue;
        }
        *pos += 1;
        if len == 0 {
            break;
        }
        let n = len as usize;
        if *pos + n > msg.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&msg[*pos..*pos + n]).into_owned());
        *pos += n;
        hops += 1;
    }
    if jumped {
        *pos = return_pos;
    }
    Some(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptr_is_first_answer() {
        let pkt = advertise_packet("VirtualFace-test", "_oscjson", "_tcp", 12345);
        assert!(pkt.len() > 12);
        let flags = u16::from_be_bytes([pkt[2], pkt[3]]);
        assert_eq!(flags, 0x8400);
        let an = u16::from_be_bytes([pkt[6], pkt[7]]);
        assert_eq!(an, 1);
        let mut pos = 12usize;
        let owner = read_name(&pkt, &mut pos).unwrap();
        assert_eq!(owner, ["_oscjson", "_tcp", "local"]);
        let rtype = u16::from_be_bytes([pkt[pos], pkt[pos + 1]]);
        assert_eq!(rtype, 12);
    }
}
