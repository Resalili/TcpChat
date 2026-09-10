use crate::protocol::{Announce,encode_announce,decode_announce};
use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use crate::peer::PeerList;

pub async fn setup_broadcast_soket(port: u16) -> std::io::Result<UdpSocket> {
    let soket = UdpSocket::bind(("0.0.0.0", port)).await?;
    soket.set_broadcast(true)?;
    Ok(soket)
}

pub async fn announce(soket: &UdpSocket, broadcast_port: u16, info: &Announce) -> std::io::Result<()> {
    let bytes = encode_announce(info);
    let addr: SocketAddr = format!("255.255.255.255:{broadcast_port}").parse().unwrap();
    soket.send_to(&bytes, addr).await?;
    Ok(())
}



pub async fn listen_for_peers(soket: Arc<UdpSocket>, peers: Arc<Mutex<PeerList>>, my_session_id: u64) {
    let mut buf = [0u8; 512];
    loop {
        match soket.recv_from(&mut buf).await {
            Ok((n, from)) => {
                let data = &buf[..n];
                if let Some(announce) = decode_announce(data) {
                    let is_new = {
                        let mut list = peers.lock().unwrap();
                        list.update(announce.session_id, announce.nickname.clone(), from, announce.tcp_port, announce.status)
                    };

                    if is_new && my_session_id > announce.session_id {
                        // TODO: ініціювати TCP-з'єднання до цього піра
                        println!("Новий пір {}, я ініціюю з'єднання", announce.nickname);
                    }
                }
            }
            Err(e) => eprintln!("помилка recv_from: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_socket_binds_successfully() {
        // порт 0 — ОС сама підбере вільний порт, щоб тести не конфліктували між собою
        let result = setup_broadcast_soket(0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn announce_roundtrip_via_unicast() {
        // два сокети на випадкових вільних портах — імітуємо двох пірів
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let receiver_addr = receiver.local_addr().unwrap();

        let info = Announce {
            session_id: 42,
            nickname: "TestPeer".to_string(),
            tcp_port: 8080,
            status: 0,
        };
        let bytes = encode_announce(&info);

        // напряму на адресу receiver, а не через announce()/broadcast —
        // так тест не залежить від того, чи доходить broadcast до loopback у цьому середовищі
        sender.send_to(&bytes, receiver_addr).await.unwrap();

        let mut buf = [0u8; 512];
        let (n, _from) = receiver.recv_from(&mut buf).await.unwrap();
        let data = &buf[..n];

        let decoded = decode_announce(data).unwrap();
        assert_eq!(decoded.nickname, "TestPeer");
        assert_eq!(decoded.tcp_port, 8080);
        assert_eq!(decoded.status, 0);
    }

    #[tokio::test]
    async fn listen_for_peers_ignores_garbage_packets() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let receiver_addr = receiver.local_addr().unwrap();

        // сміттєвий пакет, який decode_announce має відхилити (None), не панікуючи
        let garbage = vec![0xFF, 0x01, 0x02];
        sender.send_to(&garbage, receiver_addr).await.unwrap();

        let mut buf = [0u8; 512];
        let (n, _from) = receiver.recv_from(&mut buf).await.unwrap();
        let data = &buf[..n];

        assert!(decode_announce(data).is_none());
    }
}
