use std::sync::Arc;
use tokio::net::TcpListener;
use crate::network::ConnectionManager;

pub async fn run_server(port: u16, manager: Arc<ConnectionManager>) -> std::io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("TCP listener запущено на порту {port}");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("нове TCP-з'єднання від {addr}");
        let mgr = manager.clone();
        tokio::spawn(async move {
            crate::network::connection::handle_incoming(socket, addr, mgr).await;
        });
    }
}
