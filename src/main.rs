mod ui;
mod protocol;
mod discovery;
mod network;
mod peer;
mod logger;

use ui::AppEvent;
use peer::PeerList;
use protocol::Announce;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::io::{self, Write};
use tokio::sync::mpsc;

const DISCOVERY_PORT: u16 = 9999;

fn register_user() -> (String, u16) {
    print!("Введи свій нік: ");
    io::stdout().flush().unwrap();
    let mut nickname = String::new();
    io::stdin().read_line(&mut nickname).unwrap();
    let nickname = nickname.trim().to_string();

    print!("Введи TCP-порт для чату: ");
    io::stdout().flush().unwrap();
    let mut port_input = String::new();
    io::stdin().read_line(&mut port_input).unwrap();
    let tcp_port: u16 = port_input.trim().parse().expect("порт має бути числом 0-65535");

    (nickname, tcp_port)
}

fn generate_session_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
    let pid = std::process::id() as u64;
    nanos ^ pid
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    logger::init();

    let (nickname, tcp_port) = register_user();
    let session_id = generate_session_id();

    let socket = discovery::setup_broadcast_soket(DISCOVERY_PORT).await?;
    let socket = Arc::new(socket);
    let peers = Arc::new(Mutex::new(PeerList::new()));

    let (event_tx, event_rx) = mpsc::channel::<AppEvent>(64);
    let manager = Arc::new(network::ConnectionManager::new(event_tx));

    let my_info = Announce {
        session_id,
        nickname: nickname.clone(), // клонуємо: nickname ще знадобиться нижче для listen_for_peers
        tcp_port,
        status: 0,
    };

    let announce_socket = socket.clone();
    let announce_handle = tokio::spawn(async move {
        loop {
            if let Err(e) = discovery::announce(&announce_socket, DISCOVERY_PORT, &my_info).await {
                crate::error!("помилка announce: {e}");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    let listen_socket = socket.clone();
    let listen_peers = peers.clone();
    let listen_manager = manager.clone();
    let listen_handle = tokio::spawn(discovery::listen_for_peers(
        listen_socket, listen_peers, session_id, nickname.clone(), listen_manager,
    ));

    let listener_handle = tokio::spawn(network::listener::run_server(tcp_port, manager.clone()));

    let ui_manager = manager.clone();
    let ui_handle = tokio::spawn(async move {
        if let Err(e) = ui::run(event_rx, ui_manager).await {
            crate::error!("помилка UI: {e}");
        }
    });

    let _ = tokio::join!(announce_handle, listen_handle, listener_handle, ui_handle);
    Ok(())
}
