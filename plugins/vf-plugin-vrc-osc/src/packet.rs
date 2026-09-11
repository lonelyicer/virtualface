//! Encode bounded OSC datagrams, splitting only between complete messages.

use rosc::{OscMessage, OscPacket, encoder};
use vf_sdk::{Result, SdkError};

// Match the default OscCore receive buffer and VRCFaceTracking's bundle limit.
pub const MAX_DATAGRAM_BYTES: usize = 4096;
const BUNDLE_HEADER: &[u8] = b"#bundle\0\0\0\0\0\0\0\0\x01";

pub fn encode_messages(messages: Vec<OscMessage>) -> Result<Vec<Vec<u8>>> {
    let mut packets = Vec::new();
    let mut bundle = BUNDLE_HEADER.to_vec();
    for message in messages {
        let bytes = encoder::encode(&OscPacket::Message(message))
            .map_err(|e| SdkError::other(e.to_string()))?;
        if bytes.len() > MAX_DATAGRAM_BYTES {
            return Err(SdkError::other(format!(
                "OSC message is {} bytes; maximum is {MAX_DATAGRAM_BYTES}",
                bytes.len()
            )));
        }
        let element_len = 4 + bytes.len();
        if bundle.len() + element_len > MAX_DATAGRAM_BYTES && bundle.len() > BUNDLE_HEADER.len() {
            packets.push(std::mem::replace(&mut bundle, BUNDLE_HEADER.to_vec()));
        }
        if BUNDLE_HEADER.len() + element_len > MAX_DATAGRAM_BYTES {
            // A message may fit by itself but not with the bundle framing.
            packets.push(bytes);
        } else {
            bundle.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            bundle.extend_from_slice(&bytes);
        }
    }
    if bundle.len() > BUNDLE_HEADER.len() {
        packets.push(bundle);
    }
    Ok(packets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::{OscTime, OscType, decoder};

    fn flatten(packet: OscPacket, messages: &mut Vec<OscMessage>) {
        match packet {
            OscPacket::Message(m) => messages.push(m),
            OscPacket::Bundle(b) => {
                assert_eq!(
                    b.timetag,
                    OscTime {
                        seconds: 0,
                        fractional: 1
                    }
                );
                for p in b.content {
                    flatten(p, messages);
                }
            }
        }
    }

    #[test]
    fn splits_large_face_frame_without_losing_messages() {
        let messages: Vec<_> = (0..200)
            .map(|i| OscMessage {
                addr: format!("/avatar/parameters/FT/v2/Expression{i}"),
                args: vec![if i % 2 == 0 {
                    OscType::Bool(true)
                } else {
                    OscType::Float(0.35)
                }],
            })
            .collect();
        let packets = encode_messages(messages.clone()).unwrap();
        assert!(packets.len() > 1);
        let mut decoded = Vec::new();
        for bytes in packets {
            assert!(bytes.len() <= MAX_DATAGRAM_BYTES);
            let (rest, packet) = decoder::decode_udp(&bytes).unwrap();
            assert!(rest.is_empty());
            flatten(packet, &mut decoded);
        }
        assert_eq!(decoded, messages);
    }

    #[test]
    fn accepts_exact_bundle_limit_and_splits_next_message() {
        // Address '/x' + type tag + blob size + payload = 4076 bytes.
        let large = OscMessage {
            addr: "/x".into(),
            args: vec![OscType::Blob(vec![0; 4064])],
        };
        let next = OscMessage {
            addr: "/y".into(),
            args: vec![OscType::Bool(false)],
        };
        let packets = encode_messages(vec![large.clone(), next.clone()]).unwrap();
        assert_eq!(packets.iter().map(Vec::len).collect::<Vec<_>>(), [4096, 28]);
        let mut decoded = Vec::new();
        for bytes in packets {
            flatten(decoder::decode_udp(&bytes).unwrap().1, &mut decoded);
        }
        assert_eq!(decoded, [large, next]);
    }

    #[test]
    fn sends_large_individual_message_without_bundle_overhead() {
        let message = OscMessage {
            addr: "/x".into(),
            args: vec![OscType::Blob(vec![0; 4084])],
        };
        let packets = encode_messages(vec![message.clone()]).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].len(), MAX_DATAGRAM_BYTES);
        assert_eq!(
            decoder::decode_udp(&packets[0]).unwrap().1,
            OscPacket::Message(message)
        );
    }

    #[test]
    fn rejects_individual_message_over_limit() {
        let message = OscMessage {
            addr: "/x".into(),
            args: vec![OscType::Blob(vec![0; 4088])],
        };
        assert!(encode_messages(vec![message]).is_err());
        assert!(encode_messages(Vec::new()).unwrap().is_empty());
    }
}
