pub mod listener;
pub mod connection;

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use crate::ui::AppEvent;

pub struct ConnectionManager {
    connections: Mutex<HashMap<String, mpsc::Sender<String>>>,
    event_tx: mpsc::Sender<AppEvent>,
}

impl ConnectionManager {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        ConnectionManager {
            connections: Mutex::new(HashMap::new()),
            event_tx,
        }
    }

    pub fn send_event(&self, event: AppEvent) {
        // ігноруємо помилку: якщо TUI вже закрився, надсилати подію нікуди
        let _ = self.event_tx.try_send(event);
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

    // допоміжна функція: створює менеджер з каналом подій, який тест може ігнорувати
    fn test_manager() -> ConnectionManager {
        let (event_tx, _event_rx) = mpsc::channel::<AppEvent>(16);
        ConnectionManager::new(event_tx)
    }

    #[test]
    fn no_connection_by_default() {
        let manager = test_manager();
        assert!(!manager.has_connection("Bob"));
    }

    #[test]
    fn add_and_has_connection() {
        let manager = test_manager();
        let (tx, _rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);

        assert!(manager.has_connection("Bob"));
        assert!(!manager.has_connection("Alice"));
    }

    #[test]
    fn remove_connection() {
        let manager = test_manager();
        let (tx, _rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);
        assert!(manager.has_connection("Bob"));

        manager.remove("Bob");
        assert!(!manager.has_connection("Bob"));
    }

    #[tokio::test]
    async fn send_to_existing_connection_delivers_message() {
        let manager = test_manager();
        let (tx, mut rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);

        let result = manager.send_to("Bob", "привіт".to_string()).await;
        assert!(result.is_ok());

        let received = rx.recv().await;
        assert_eq!(received, Some("привіт".to_string()));
    }

    #[tokio::test]
    async fn send_to_unknown_peer_returns_error() {
        let manager = test_manager();

        let result = manager.send_to("Ghost", "привіт".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_to_closed_channel_returns_error() {
        let manager = test_manager();
        let (tx, rx) = mpsc::channel::<String>(1);

        manager.add("Bob".to_string(), tx);
        drop(rx);

        let result = manager.send_to("Bob", "привіт".to_string()).await;
        assert!(result.is_err());
    }
}
