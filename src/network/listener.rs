use tokio::net::TcpListener;

pub async fn run_server(port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("TCP listener запущено на порту {port}");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("нове TCP-з'єднання від {addr}");
        tokio::spawn(crate::network::connection::handle_connection(socket));
    }
}
