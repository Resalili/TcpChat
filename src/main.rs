mod protocol;
mod discovery;
mod network;

use protocol::Announce;
use std::sync::Arc;
use std::time::Duration;
use std::io::{self, Write};

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

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (nickname, tcp_port) = register_user();

    let socket = discovery::setup_broadcast_soket(DISCOVERY_PORT).await?;
    let socket = Arc::new(socket);

    let my_info = Announce {
        nickname,
        tcp_port,
        status: 0,
    };

    let announce_socket = socket.clone();
    let announce_handle = tokio::spawn(async move {
        loop {
            if let Err(e) = discovery::announce(&announce_socket, DISCOVERY_PORT, &my_info).await {
                eprintln!("помилка announce: {e}");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    let listen_socket = socket.clone();
    let listen_handle = tokio::spawn(discovery::listen_for_peers(listen_socket));

    let _ = tokio::join!(announce_handle, listen_handle);
    Ok(())
}
