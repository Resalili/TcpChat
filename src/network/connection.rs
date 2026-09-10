use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::protocol::{PACKET_TEXT, PACKET_IMAGE};

pub enum IncomingPacket {
    Text(String),
    Image(Vec<u8>), // сирі байти фото — розбереш формат (PNG/JPEG), коли дійдеш до цього
}

async fn read_packet(socket: &mut TcpStream) -> std::io::Result<Option<IncomingPacket>> {
    let mut type_buf = [0u8; 1];
    match socket.read_exact(&mut type_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let packet_type = type_buf[0];

    let mut len_buf = [0u8; 4];
    socket.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;

    let mut body = vec![0u8; len];
    socket.read_exact(&mut body).await?;

    match packet_type {
        PACKET_TEXT => {
            let text = String::from_utf8(body).unwrap_or_default();
            Ok(Some(IncomingPacket::Text(text)))
        }
        PACKET_IMAGE => Ok(Some(IncomingPacket::Image(body))),
        _ => {
            eprintln!("невідомий тип пакета: {packet_type}");
            Ok(None) // або продовжити цикл і читати наступний пакет — залежно від бажаної поведінки
        }
    }
}

pub async fn send_text(socket: &mut TcpStream, text: &str) -> std::io::Result<()> {
    send_packet(socket, PACKET_TEXT, text.as_bytes()).await
}

pub async fn send_image(socket: &mut TcpStream, image_bytes: &[u8]) -> std::io::Result<()> {
    send_packet(socket, PACKET_IMAGE, image_bytes).await
}

async fn send_packet(socket: &mut TcpStream, packet_type: u8, body: &[u8]) -> std::io::Result<()> {
    socket.write_all(&[packet_type]).await?;
    socket.write_all(&(body.len() as u32).to_le_bytes()).await?;
    socket.write_all(body).await?;
    Ok(())
}

pub async fn handle_connection(mut socket: TcpStream) {
    loop {
        match read_packet(&mut socket).await {
            Ok(Some(IncomingPacket::Text(text))) => {
                println!("текст: {text}");
                // TODO: передати в UI через mpsc
            }
            Ok(Some(IncomingPacket::Image(bytes))) => {
                println!("отримано фото, {} байтів", bytes.len());
                // TODO: зберегти/показати
            }
            Ok(None) => {
                println!("з'єднання закрито");
                break;
            }
            Err(e) => {
                eprintln!("помилка читання: {e}");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn setup_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let connect_fut = TcpStream::connect(addr);
        let accept_fut = listener.accept();

        let (client_result, accept_result) = tokio::join!(connect_fut, accept_fut);

        let client = client_result.unwrap();
        let (server, _) = accept_result.unwrap();

        (client, server)
    }

    #[tokio::test]
    async fn text_message_roundtrip() {
        let (mut client, mut server) = setup_pair().await;

        send_text(&mut client, "привіт").await.unwrap();

        let received = read_packet(&mut server).await.unwrap();
        match received {
            Some(IncomingPacket::Text(text)) => assert_eq!(text, "привіт"),
            _ => panic!("очікували Text-пакет"),
        }
    }

    #[tokio::test]
    async fn image_message_roundtrip() {
        let (mut client, mut server) = setup_pair().await;

        let fake_image = vec![0xDE, 0xAD, 0xBE, 0xEF];
        send_image(&mut client, &fake_image).await.unwrap();

        let received = read_packet(&mut server).await.unwrap();
        match received {
            Some(IncomingPacket::Image(bytes)) => assert_eq!(bytes, fake_image),
            _ => panic!("очікували Image-пакет"),
        }
    }

    #[tokio::test]
    async fn closed_connection_returns_none() {
        let (client, mut server) = setup_pair().await;
        drop(client); // закриваємо з'єднання з боку клієнта, нічого не надіславши

        let received = read_packet(&mut server).await.unwrap();
        assert!(received.is_none());
    }

    #[tokio::test]
    async fn multiple_messages_in_sequence() {
        let (mut client, mut server) = setup_pair().await;

        send_text(&mut client, "перше").await.unwrap();
        send_text(&mut client, "друге").await.unwrap();

        let first = read_packet(&mut server).await.unwrap();
        let second = read_packet(&mut server).await.unwrap();

        match (first, second) {
            (Some(IncomingPacket::Text(a)), Some(IncomingPacket::Text(b))) => {
                assert_eq!(a, "перше");
                assert_eq!(b, "друге");
            }
            _ => panic!("очікували два Text-пакети поспіль"),
        }
    }
}
