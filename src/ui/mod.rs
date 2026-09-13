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
use crate::network::ConnectionManager;

pub enum AppEvent {
    NewPeer(String),
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
            AppEvent::NewPeer(nick) => {
                if !self.chats.contains(&nick) {
                    self.chats.push(nick.clone());
                    self.messages.insert(nick, Vec::new());
                }
            }
            AppEvent::IncomingText { from, text } => {
                self.messages.entry(from).or_default().push(text);
            }
            AppEvent::IncomingImage { from, size } => {
                self.messages.entry(from).or_default().push(format!("[фото, {size} байтів]"));
            }
            AppEvent::ConnectionClosed(nick) | AppEvent::PeerLeft(nick) => {
                let _ = nick; // поки що нічого не робимо, статус "офлайн" додамо пізніше
            }
        }
    }
}

pub async fn run(mut event_rx: mpsc::Receiver<AppEvent>, manager: Arc<ConnectionManager>) -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut key_events = EventStream::new();

    while !app.should_quit {
        terminal.draw(|f| draw_ui(f, &app))?;

        tokio::select! {
            Some(Ok(term_event)) = key_events.next() => {
                if let CEvent::Key(key) = term_event {
                    handle_key(&mut app, key.code, &manager).await;
                }
                // CEvent::Resize/Mouse/FocusGained тощо — нічого не робимо всередині,
                // але сам факт спрацювання цієї гілки select! призведе до нового terminal.draw() на початку циклу
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

async fn handle_key(app: &mut App, code: KeyCode, manager: &Arc<ConnectionManager>) {
    match app.mode {
        Mode::Navigate => match code {
            KeyCode::Char('q') => app.should_quit = true,
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
                                eprintln!("не вдалось надіслати: {e}");
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

fn draw_ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    let items: Vec<ListItem> = app.chats.iter().enumerate().map(|(i, nick)| {
        let style = if i == app.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        ListItem::new(nick.as_str()).style(style)
    }).collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Чати"));
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
