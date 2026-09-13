pub enum AppEvent {
    NewPeer(String),
    PeerLeft(String),
    IncomingText { from: String, text: String },
    IncomingImage { from: String, size: usize },
    ConnectionClosed(String),
}
