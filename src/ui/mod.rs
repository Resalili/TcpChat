use ratatui::{
    Terminal, Frame,
    backend::CrosstermBackend,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    style::{Style, Modifier},
};
use crossterm::event::{EventStream, Event as CEvent, KeyCode};
use tokio_stream::StreamExt;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use crate::network::ConnectionManager;
use crate::protocol::Status;

pub enum AppEvent {
    PeerUpdate { nick: String, status: Status },
    PeerLeft(String),
    IncomingText { from: String, text: String },
    IncomingImage { from: String, size: usize },
    ConnectionClosed(String),
}

#[derive(PartialEq)]
pub enum Mode {
    Navigate,
    Chatting,
}

pub struct App {
    pub chats: Vec<String>,
    pub statuses: std::collections::HashMap<String, Status>,
    pub selected: usize,
    pub active: Option<usize>,
    pub messages: std::collections::HashMap<String, Vec<String>>,
    pub input: String,
    pub mode: Mode,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            chats: Vec::new(),
            statuses: std::collections::HashMap::new(),
            selected: 0,
            active: None,
            messages: std::collections::HashMap::new(),
            input: String::new(),
            mode: Mode::Navigate,
            should_quit: false,
        }
    }

    fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::PeerUpdate { nick, status } => {
                if !self.chats.contains(&nick) {
                    self.chats.push(nick.clone());
                    self.messages.insert(nick.clone(), Vec::new());
                }
                self.statuses.insert(nick, status);
            }
            AppEvent::PeerLeft(nick) => {
                self.chats.retain(|n| n != &nick);
                self.messages.remove(&nick);
                self.statuses.remove(&nick);
                if let Some(active_idx) = self.active {
                    if self.chats.get(active_idx).map(|n| n == &nick).unwrap_or(true) {
                        self.active = None;
                    }
                }
            }
            AppEvent::IncomingText { from, text } => {
                self.messages.entry(from.clone()).or_default().push(format!("{from}: {text}"));
            }
            AppEvent::IncomingImage { from, size } => {
                self.messages.entry(from).or_default().push(format!("[фото, {size} байтів]"));
            }
            AppEvent::ConnectionClosed(nick) => {
                self.messages.entry(nick).or_default().push("-- з'єднання розірвано --".to_string());
            }
        }
    }
}

pub async fn run(
    mut event_rx: mpsc::Receiver<AppEvent>,
    manager: Arc<ConnectionManager>,
    my_status: Arc<AtomicU8>,
) -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut key_events = EventStream::new();

    while !app.should_quit {
        terminal.draw(|f| draw_ui(f, &app, &my_status))?;

        tokio::select! {
            Some(Ok(term_event)) = key_events.next() => {
                if let CEvent::Key(key) = term_event {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        handle_key(&mut app, key.code, &manager, &my_status).await;
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                app.handle_app_event(event);
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}

async fn handle_key(app: &mut App, code: KeyCode, manager: &Arc<ConnectionManager>, my_status: &Arc<AtomicU8>) {
    match app.mode {
        Mode::Navigate => match code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('s') => {
                let current = Status::from_u8(my_status.load(Ordering::Relaxed));
                let next = current.next();
                my_status.store(next.to_u8(), Ordering::Relaxed);
            }
            KeyCode::Char('j') => {
                if !app.chats.is_empty() {
                    app.selected = (app.selected + 1).min(app.chats.len() - 1);
                }
            }
            KeyCode::Char('k') => {
                app.selected = app.selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                if !app.chats.is_empty() {
                    app.active = Some(app.selected);
                    app.mode = Mode::Chatting;
                }
            }
            _ => {}
        },
        Mode::Chatting => match code {
            KeyCode::Esc => {
                app.mode = Mode::Navigate;
            }
            KeyCode::Enter => {
                if let Some(idx) = app.active {
                    let nickname = app.chats[idx].clone();
                    let text = std::mem::take(&mut app.input);
                    if !text.is_empty() {
                        app.messages.entry(nickname.clone()).or_default().push(format!("я: {text}"));
                        let mgr = manager.clone();
                        tokio::spawn(async move {
                            if let Err(e) = mgr.send_to(&nickname, text).await {
                                crate::error!("не вдалось надіслати: {e}");
                            }
                        });
                    }
                }
            }
            KeyCode::Char(c) => app.input.push(c),
            KeyCode::Backspace => { app.input.pop(); }
            _ => {}
        },
    }
}

fn status_letter(status: Status) -> &'static str {
    match status {
        Status::Online => "[О]",
        Status::Away => "[В]",
        Status::Invisible => "[Н]",
    }
}

fn draw_ui(f: &mut Frame, app: &App, my_status: &Arc<AtomicU8>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    let items: Vec<ListItem> = app.chats.iter().enumerate().map(|(i, nick)| {
        let status = app.statuses.get(nick).copied().unwrap_or(Status::Online);
        let label = format!("{} {}", status_letter(status), nick);
        let style = if i == app.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        ListItem::new(label).style(style)
    }).collect();

    let my_status_val = Status::from_u8(my_status.load(Ordering::Relaxed));
    let list_title = format!("Чати (я: {}) — 's' змінити статус", status_letter(my_status_val));
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(list_title));
    f.render_widget(list, chunks[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunks[1]);

    let title = app.active.map(|i| app.chats[i].as_str()).unwrap_or("(нема активного чату)");
    let body = app.active
        .and_then(|i| app.messages.get(&app.chats[i]))
        .map(|msgs| msgs.join("\n"))
        .unwrap_or_default();
    let chat_view = Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(chat_view, right[0]);

    let input_style = if app.mode == Mode::Chatting {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let input = Paragraph::new(app.input.as_str())
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title("Ввід (Enter — надіслати, Esc — до списку)"));
    f.render_widget(input, right[1]);
}
