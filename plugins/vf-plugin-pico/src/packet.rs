//! PICO binary packet parsing (new protocol with 16-byte header, legacy body-only).

pub const PICO_SHAPE_COUNT: usize = 72;
pub const HEADER_SIZE: usize = 16;
pub const BODY_WEIGHTS_OFFSET: usize = 8; // skip i64 timestamp
pub const BODY_MIN: usize = BODY_WEIGHTS_OFFSET + PICO_SHAPE_COUNT * 4;

#[derive(Clone, Debug)]
pub struct PicoFrame {
    pub timestamp: u64,
    pub tracking_type: u8,
    pub legacy: bool,
    pub weights: [f32; PICO_SHAPE_COUNT],
}

pub fn parse_pico_packet(data: &[u8]) -> Option<PicoFrame> {
    if data.len() >= HEADER_SIZE + BODY_MIN {
        let tracking_type = data[2];
        if tracking_type == 2 {
            return parse_body(&data[HEADER_SIZE..], false, tracking_type);
        }
    }
    if data.len() >= BODY_MIN {
        return parse_body(data, true, 0);
    }
    None
}

fn parse_body(body: &[u8], legacy: bool, tracking_type: u8) -> Option<PicoFrame> {
    if body.len() < BODY_MIN {
        return None;
    }
    let timestamp = u64::from_le_bytes(body[0..8].try_into().ok()?);
    let mut weights = [0f32; PICO_SHAPE_COUNT];
    for i in 0..PICO_SHAPE_COUNT {
        let s = BODY_WEIGHTS_OFFSET + i * 4;
        weights[i] = f32::from_le_bytes(body[s..s + 4].try_into().ok()?);
    }
    Some(PicoFrame {
        timestamp,
        tracking_type,
        legacy,
        weights,
    })
}

pub fn encode_new_packet(weights: &[f32; PICO_SHAPE_COUNT], timestamp: u64) -> Vec<u8> {
    let mut buf = vec![0u8; HEADER_SIZE + BODY_MIN];
    buf[0] = 0x50;
    buf[1] = 0x46;
    buf[2] = 2; // trackingType face
    buf[HEADER_SIZE..HEADER_SIZE + 8].copy_from_slice(&timestamp.to_le_bytes());
    for i in 0..PICO_SHAPE_COUNT {
        let s = HEADER_SIZE + BODY_WEIGHTS_OFFSET + i * 4;
        buf[s..s + 4].copy_from_slice(&weights[i].to_le_bytes());
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_new_packet() {
        let mut w = [0f32; PICO_SHAPE_COUNT];
        w[7] = 0.42; // JawOpen in pico index
        w[28] = 0.1;
        let bytes = encode_new_packet(&w, 12345);
        let parsed = parse_pico_packet(&bytes).unwrap();
        assert!(!parsed.legacy);
        assert_eq!(parsed.tracking_type, 2);
        assert!((parsed.weights[7] - 0.42).abs() < 1e-6);
        assert!((parsed.weights[28] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn legacy_body() {
        let mut body = vec![0u8; BODY_MIN];
        body[0..8].copy_from_slice(&99u64.to_le_bytes());
        body[BODY_WEIGHTS_OFFSET..BODY_WEIGHTS_OFFSET + 4].copy_from_slice(&0.5f32.to_le_bytes());
        let parsed = parse_pico_packet(&body).unwrap();
        assert!(parsed.legacy);
        assert!((parsed.weights[0] - 0.5).abs() < 1e-6);
    }
}
