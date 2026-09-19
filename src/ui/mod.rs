use ratatui::{
    Terminal, Frame,
    backend::CrosstermBackend,
    layout::{Layout, Direction, Constraint},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    style::{Style, Modifier, Color},
    text::{Line, Span},
};
use crossterm::event::{EventStream, Event as CEvent, KeyCode};
use tokio_stream::StreamExt;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::collections::HashMap;
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

pub enum LineKind {
    Own,
    Incoming,
    System,
}

pub struct ChatLine {
    pub text: String,
    pub kind: LineKind,
}

pub struct App {
    pub chats: Vec<String>,
    pub statuses: HashMap<String, Status>,
    pub unread: HashMap<String, usize>,
    pub selected: usize,
    pub active: Option<usize>,
    pub messages: HashMap<String, Vec<ChatLine>>,
    pub input: String,
    pub mode: Mode,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            chats: Vec::new(),
            statuses: HashMap::new(),
            unread: HashMap::new(),
            selected: 0,
            active: None,
            messages: HashMap::new(),
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
                self.unread.remove(&nick);
                if let Some(active_idx) = self.active {
                    if self.chats.get(active_idx).map(|n| n == &nick).unwrap_or(true) {
                        self.active = None;
                    }
                }
            }
            AppEvent::IncomingText { from, text } => {
                let currently_open = self.active.and_then(|i| self.chats.get(i)) == Some(&from);
                if !currently_open {
                    *self.unread.entry(from.clone()).or_insert(0) += 1;
                }
                self.messages.entry(from.clone()).or_default().push(ChatLine {
                    text: format!("{from}: {text}"),
                    kind: LineKind::Incoming,
                });
            }
            AppEvent::IncomingImage { from, size } => {
                let currently_open = self.active.and_then(|i| self.chats.get(i)) == Some(&from);
                if !currently_open {
                    *self.unread.entry(from.clone()).or_insert(0) += 1;
                }
                self.messages.entry(from.clone()).or_default().push(ChatLine {
                    text: format!("{from}: [фото, {size} байтів]"),
                    kind: LineKind::Incoming,
                });
            }
            AppEvent::ConnectionClosed(nick) => {
                self.messages.entry(nick).or_default().push(ChatLine {
                    text: "-- з'єднання розірвано --".to_string(),
                    kind: LineKind::System,
                });
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
                    if let Some(nick) = app.chats.get(app.selected) {
                        app.unread.remove(nick);
                    }
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
                        app.messages.entry(nickname.clone()).or_default().push(ChatLine {
                            text: format!("я: {text}"),
                            kind: LineKind::Own,
                        });
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

fn status_color(status: Status) -> Color {
    match status {
        Status::Online => Color::Green,
        Status::Away => Color::Yellow,
        Status::Invisible => Color::DarkGray,
    }
}

fn draw_ui(f: &mut Frame, app: &App, my_status: &Arc<AtomicU8>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    let list_focused = app.mode == Mode::Navigate;
    let chat_focused = app.mode == Mode::Chatting;
    let focused_border = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let normal_border = Style::default();

    let items: Vec<ListItem> = app.chats.iter().enumerate().map(|(i, nick)| {
        let status = app.statuses.get(nick).copied().unwrap_or(Status::Online);
        let unread = app.unread.get(nick).copied().unwrap_or(0);

        let mut spans = vec![
            Span::styled("● ", Style::default().fg(status_color(status))),
            Span::raw(nick.clone()),
        ];
        if unread > 0 {
            spans.push(Span::styled(format!("  ({unread})"), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
        }

        let base_style = if i == app.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(spans)).style(base_style)
    }).collect();

    let my_status_val = Status::from_u8(my_status.load(Ordering::Relaxed));
    let list_title = Line::from(vec![
        Span::raw("Чати "),
        Span::styled("●", Style::default().fg(status_color(my_status_val))),
    ]);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if list_focused { focused_border } else { normal_border })
            .title(list_title)
    );
    f.render_widget(list, chunks[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3), Constraint::Length(1)])
        .split(chunks[1]);

    let title = app.active.map(|i| app.chats[i].as_str()).unwrap_or("(нема активного чату)");
    let lines: Vec<Line> = app.active
        .and_then(|i| app.messages.get(&app.chats[i]))
        .map(|msgs| msgs.iter().map(|line| {
            let color = match line.kind {
                LineKind::Own => Color::Cyan,
                LineKind::Incoming => Color::White,
                LineKind::System => Color::DarkGray,
            };
            Line::styled(line.text.clone(), Style::default().fg(color))
        }).collect())
        .unwrap_or_default();

    let chat_view = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if chat_focused { focused_border } else { normal_border })
                .title(title)
        );
    f.render_widget(chat_view, right[0]);

    let input_style = if app.mode == Mode::Chatting {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let input = Paragraph::new(app.input.as_str())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if chat_focused { focused_border } else { normal_border })
                .title("Ввід")
        );
    f.render_widget(input, right[1]);

    let hint = match app.mode {
        Mode::Navigate => "j/k: рух  Enter: відкрити чат  s: змінити статус  q: вихід",
        Mode::Chatting => "Enter: надіслати  Esc: до списку чатів",
    };
    let hint_bar = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint_bar, right[2]);
}
