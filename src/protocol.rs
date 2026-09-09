pub const PACKET_ANNOUNCE: u8 = 0x01;

pub struct Announce{
    pub nickname: String,
    pub tcp_port: u16,
    pub status: u8,
}

pub fn encode_announce(a: &Announce) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(PACKET_ANNOUNCE);
    let name_bytes = a.nickname.as_bytes();

    buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(name_bytes);
    buf.extend_from_slice(&a.tcp_port.to_le_bytes());
    buf.push(a.status);

    buf
}

pub fn decode_announce(data: &[u8]) -> Option<Announce> {
    if data.is_empty() || data[0] != PACKET_ANNOUNCE {
        return None;
    }

    let name_len_bytes = data.get(1..3)?;
    let name_len = u16::from_le_bytes(name_len_bytes.try_into().unwrap()) as usize;

    let name_bytes = data.get(3..3 + name_len)?;
    let nickname = String::from_utf8(name_bytes.to_vec()).ok()?;

    let port_bytes = data.get(3 + name_len..5 + name_len)?;
    let tcp_port = u16::from_le_bytes(port_bytes.try_into().unwrap());

    let status = *data.get(5 + name_len)?;

    Some(Announce { nickname, tcp_port, status })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announce_encode_decode() {
        let announce = Announce {
            nickname: "Artem".to_string(),
            tcp_port: 8080,
            status: 1,
        };

        let encoded = encode_announce(&announce);
        let decoded = decode_announce(&encoded).unwrap();

        assert_eq!(decoded.nickname, announce.nickname);
        assert_eq!(decoded.tcp_port, announce.tcp_port);
        assert_eq!(decoded.status, announce.status);
    }
    #[test]
    fn announce_decoding() {
        let data = vec![
            0x01,
            0x03, 0x00,
            b'B', b'o', b'b',
            0x90, 0x1F,
            0x02,
        ];

        let announce = decode_announce(&data).unwrap();

        assert_eq!(announce.nickname, "Bob");
        assert_eq!(announce.tcp_port, 8080);
        assert_eq!(announce.status, 2);
    }
    #[test]
    fn announce_encoding() {
        let announce = Announce {
            nickname: "Bob".to_string(),
            tcp_port: 8080,
            status: 2,
        };

        let encoded = encode_announce(&announce);

        assert_eq!(
            encoded,
            vec![
                0x01,       // packet type
                0x03, 0x00, // nickname length
                b'B', b'o', b'b',
                0x90, 0x1F, // 8080 LE
                0x02,       // status
            ]
        );
    }
}

