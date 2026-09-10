pub mod listener;
pub mod connection;

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct ConnectionManager {
    connections: Mutex<HashMap<String, mpsc::Sender<String>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager { connections: Mutex::new(HashMap::new()) }
    }

    pub fn has_connection(&self, nickname: &str) -> bool {
        self.connections.lock().unwrap().contains_key(nickname)
    }

    pub fn add(&self, nickname: String, sender: mpsc::Sender<String>) {
        self.connections.lock().unwrap().insert(nickname, sender);
    }

    pub fn remove(&self, nickname: &str) {
        self.connections.lock().unwrap().remove(nickname);
    }

    pub async fn send_to(&self, nickname: &str, text: String) -> Result<(), &'static str> {
        let sender = {
            let map = self.connections.lock().unwrap();
            map.get(nickname).cloned()
        };
        match sender {
            Some(tx) => tx.send(text).await.map_err(|_| "з'єднання закрито"),
            None => Err("немає з'єднання з цим піром"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_connection_by_default() {
        let manager = ConnectionManager::new();
        assert!(!manager.has_connection("Bob"));
    }

    #[test]
    fn add_and_has_connection() {
        let manager = ConnectionManager::new();
        let (tx, _rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);

        assert!(manager.has_connection("Bob"));
        assert!(!manager.has_connection("Alice")); // інший нік — не повинен з'явитись
    }

    #[test]
    fn remove_connection() {
        let manager = ConnectionManager::new();
        let (tx, _rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);
        assert!(manager.has_connection("Bob"));

        manager.remove("Bob");
        assert!(!manager.has_connection("Bob"));
    }

    #[tokio::test]
    async fn send_to_existing_connection_delivers_message() {
        let manager = ConnectionManager::new();
        let (tx, mut rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);

        let result = manager.send_to("Bob", "привіт".to_string()).await;
        assert!(result.is_ok());

        let received = rx.recv().await;
        assert_eq!(received, Some("привіт".to_string()));
    }

    #[tokio::test]
    async fn send_to_unknown_peer_returns_error() {
        let manager = ConnectionManager::new();

        let result = manager.send_to("Ghost", "привіт".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_closed_channel_returns_error() {
        let manager = ConnectionManager::new();
        let (tx, rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);
        drop(rx); // отримувач закрився — канал "мертвий" з боку відправника

        let result = manager.send_to("Bob", "привіт".to_string()).await;
        assert!(result.is_err());
    }
}
