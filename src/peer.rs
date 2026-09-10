use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

pub struct Peer {
    pub session_id: u64,
    pub nickname: String,
    pub addr: SocketAddr,
    pub tcp_port: u16,
    pub status: u8,
    pub last_seen: Instant,
}

pub struct PeerList {
    peers: HashMap<String, Peer>,
}

impl PeerList {
    pub fn new() -> Self {
        PeerList { peers: HashMap::new() }
    }

    pub fn update(&mut self,session_id: u64, nickname: String, addr: SocketAddr, tcp_port: u16, status: u8) {
        self.peers.insert(nickname.clone(), Peer {
            session_id,
            nickname,
            addr,
            tcp_port,
            status,
            last_seen: Instant::now(),
        });
    }

    pub fn remove_stale(&mut self, timeout: std::time::Duration) {
        self.peers.retain(|_, peer| peer.last_seen.elapsed() < timeout);
    }

    pub fn list(&self) -> impl Iterator<Item = &Peer> {
        self.peers.values()
    }
}
