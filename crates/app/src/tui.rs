use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};
use tokio::sync::watch;

use crate::i18n;
use crate::state_monitor::ConnectionStatus;

// ===== Catppuccin Mocha Palette =====
mod color {
    use ratatui::style::Color;
    pub const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
    pub const SURFACE0: Color = Color::Rgb(0x31, 0x32, 0x44);
    pub const SURFACE1: Color = Color::Rgb(0x45, 0x46, 0x5a);
    pub const OVERLAY0: Color = Color::Rgb(0x6c, 0x70, 0x86);
    pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
    pub const SUBTEXT0: Color = Color::Rgb(0xa6, 0xad, 0xc8);
    pub const BLUE: Color = Color::Rgb(0x89, 0xb4, 0xfa);
    pub const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);
    pub const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
    pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
    pub const MAUVE: Color = Color::Rgb(0xcb, 0xa6, 0xf7);
    pub const PEACH: Color = Color::Rgb(0xfa, 0xb3, 0x87);
}

// ===== Shared Log Buffer =====

#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogEntry>>>,
    max: usize,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub level: &'static str,
    pub message: String,
}

impl LogBuffer {
    pub fn new(max: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(max))),
            max,
        }
    }

    pub fn push(&self, level: &'static str, message: String) {
        let mut buf = self.inner.lock().unwrap();
        if buf.len() >= self.max {
            buf.pop_front();
        }
        buf.push_back(LogEntry { level, message });
    }

    pub fn push_str(&self, level: &'static str, msg: impl std::fmt::Display) {
        self.push(level, msg.to_string());
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().iter().rev().take(50).cloned().collect()
    }
}

// ===== Application State =====

struct GameStateDisplay {
    scene: String,
    is_own_turn: bool,
    turn: u32,
    own_hp: i32,
    own_armor: i32,
    own_max_hp: i32,
    enemy_hp: i32,
    enemy_armor: i32,
    mana: u32,
    max_mana: u32,
    hand_count: u32,
    own_minion_count: usize,
    enemy_minion_count: usize,
    own_deck_count: u32,
    enemy_hand_count: u32,
}

impl Default for GameStateDisplay {
    fn default() -> Self {
        Self {
            scene: "Unknown".into(),
            is_own_turn: false,
            turn: 0,
            own_hp: 0, own_armor: 0, own_max_hp: 0,
            enemy_hp: 0, enemy_armor: 0,
            mana: 0, max_mana: 0,
            hand_count: 0,
            own_minion_count: 0, enemy_minion_count: 0,
            own_deck_count: 30, enemy_hand_count: 0,
        }
    }
}

struct SessionStats {
    games: u32, wins: u32, losses: u32,
}

impl Default for SessionStats {
    fn default() -> Self { Self { games: 0, wins: 0, losses: 0 } }
}

pub struct TuiApp {
    state_rx: watch::Receiver<Option<hb_ipc::GameStateData>>,
    status_rx: watch::Receiver<ConnectionStatus>,
    log_buf: LogBuffer,
    game_state: GameStateDisplay,
    connection: ConnectionStatus,
    stats: SessionStats,
    current_lang: u8,
    tab_index: usize,
}

impl TuiApp {
    pub fn new(
        state_rx: watch::Receiver<Option<hb_ipc::GameStateData>>,
        status_rx: watch::Receiver<ConnectionStatus>,
        log_buf: LogBuffer,
    ) -> Self {
        Self {
            state_rx,
            status_rx,
            log_buf,
            game_state: GameStateDisplay::default(),
            connection: ConnectionStatus::Connected,
            stats: SessionStats::default(),
            current_lang: 0,
            tab_index: 0,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('l') | KeyCode::Char('L') => self.toggle_lang(),
            KeyCode::Tab => self.tab_index = (self.tab_index + 1) % 3,
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.log_buf.push_str("info", "Session reset requested");
            }
            _ => {}
        }
    }

    fn toggle_lang(&mut self) {
        self.current_lang = 1 - self.current_lang;
        let lang_str = if self.current_lang == 1 { "zh" } else { "en" };
        i18n::set_language(lang_str);
        self.log_buf.push_str("info", format!("Language: {lang_str}"));
    }

    fn pull_state(&mut self) {
        let snapshot = self.state_rx.borrow().clone();
        if let Some(state) = snapshot {
            self.game_state = GameStateDisplay {
                scene: state.scene,
                is_own_turn: state.is_own_turn,
                turn: state.turn,
                own_hp: state.own_hero.health,
                own_armor: state.own_hero.armor,
                own_max_hp: state.own_hero.health.max(1),
                enemy_hp: state.enemy_hero.health,
                enemy_armor: state.enemy_hero.armor,
                mana: state.own_mana,
                max_mana: state.own_max_mana,
                hand_count: state.own_hand_count,
                own_minion_count: state.own_minions.len(),
                enemy_minion_count: state.enemy_minions.len(),
                own_deck_count: state.own_deck_count,
                enemy_hand_count: state.enemy_hand_count,
            };
        }
    }

    fn pull_connection(&mut self) {
        self.connection = self.status_rx.borrow().clone();
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Min(6),
                Constraint::Length(1),
            ])
            .spacing(0)
            .split(area);
        self.render_header(frame, vert[0]);
        self.render_body(frame, vert[1]);
        self.render_log(frame, vert[2]);
        self.render_footer(frame, vert[3]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let connected = matches!(self.connection, ConnectionStatus::Connected);
        let dot = if connected { "●" } else { "●" };
        let dot_color = if connected { color::GREEN } else { color::RED };
        let status_text = if connected {
            i18n::tr("status.connected")
        } else {
            i18n::tr("status.disconnected")
        };
        let lang_label = if self.current_lang == 1 { "中文" } else { "EN" };

        let line = Line::from(vec![
            Span::styled(" HB ", Style::default().fg(color::MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled(i18n::tr("app.title"), Style::default().fg(color::TEXT)),
            Span::raw("  "),
            Span::styled(dot, Style::default().fg(dot_color)),
            Span::raw(" "),
            Span::styled(status_text, Style::default().fg(color::SUBTEXT0)),
            Span::raw("  "),
            Span::styled(lang_label, Style::default().fg(color::MAUVE)),
        ]);

        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(color::SURFACE0)),
            area,
        );
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        let horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
            .spacing(1)
            .split(area);
        self.render_game_state_panel(frame, horiz[0]);
        self.render_side_panel(frame, horiz[1]);
    }

    fn render_game_state_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" {} ", i18n::tr("panel.game_state")))
            .title_style(Style::default().fg(color::BLUE).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color::SURFACE1))
            .style(Style::default().bg(color::SURFACE0));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let gs = &self.game_state;
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .spacing(1)
            .split(inner);

        let left = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("hp")), Style::default().fg(color::SUBTEXT0)),
                Span::styled(format!("{}/{}", gs.own_hp, gs.own_max_hp),
                    Style::default().fg(color::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(format!("🛡{}", gs.own_armor), Style::default().fg(color::SUBTEXT0)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("mana")), Style::default().fg(color::SUBTEXT0)),
                Span::styled(format!("{}/{}", gs.mana, gs.max_mana),
                    Style::default().fg(color::BLUE).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("turn")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{}", gs.turn)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("scene")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(&gs.scene),
            ]),
        ]).style(Style::default().bg(color::SURFACE0));
        frame.render_widget(left, cols[0]);

        let right = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("hand")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{}", gs.hand_count)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("board")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{} minions", gs.own_minion_count)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("enemy_board")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{} minions", gs.enemy_minion_count)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("deck")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{} cards", gs.own_deck_count)),
            ]),
        ]).style(Style::default().bg(color::SURFACE0));
        frame.render_widget(right, cols[1]);
    }

    fn render_side_panel(&self, frame: &mut Frame, area: Rect) {
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .spacing(1)
            .split(area);
        self.render_session_panel(frame, vert[0]);
        self.render_connection_panel(frame, vert[1]);
    }

    fn render_session_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" {} ", i18n::tr("panel.session")))
            .title_style(Style::default().fg(color::YELLOW).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color::SURFACE1))
            .style(Style::default().bg(color::SURFACE0));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let st = &self.stats;
        let wr = if st.games > 0 { st.wins as f32 / st.games as f32 * 100.0 } else { 0.0 };

        let p = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("games")), Style::default().fg(color::SUBTEXT0)),
                Span::raw(format!("{}", st.games)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("wins")), Style::default().fg(color::GREEN)),
                Span::raw(format!("{}", st.wins)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("losses")), Style::default().fg(color::RED)),
                Span::raw(format!("{}", st.losses)),
            ]),
            Line::from(vec![
                Span::styled(format!("{} ", i18n::tr("winrate")), Style::default().fg(color::SUBTEXT0)),
                Span::styled(format!("{:.0}%", wr), Style::default().fg(color::YELLOW)),
            ]),
        ]).style(Style::default().bg(color::SURFACE0));
        frame.render_widget(p, inner);
    }

    fn render_connection_panel(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" {} ", i18n::tr("panel.connection")))
            .title_style(Style::default().fg(color::MAUVE).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color::SURFACE1))
            .style(Style::default().bg(color::SURFACE0));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let connected = matches!(self.connection, ConnectionStatus::Connected);
        let sc = if connected { color::GREEN } else { color::RED };
        let st = if connected { i18n::tr("status.connected") } else { i18n::tr("status.disconnected") };

        let p = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("● ", Style::default().fg(sc)),
                Span::styled(st, Style::default().fg(color::TEXT)),
            ]),
            Line::from(vec![
                Span::styled(i18n::tr("status.ai_idle"), Style::default().fg(color::SUBTEXT0)),
            ]),
        ]).style(Style::default().bg(color::SURFACE0));
        frame.render_widget(p, inner);
    }

    fn render_log(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" {} ", i18n::tr("panel.log")))
            .title_style(Style::default().fg(color::PEACH).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color::SURFACE1))
            .style(Style::default().bg(color::SURFACE0));

        let entries = self.log_buf.snapshot();
        let items: Vec<ListItem> = entries.iter().map(|e| {
            let lc = match e.level { "error" => color::RED, "warn" => color::YELLOW, _ => color::SUBTEXT0 };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", e.level), Style::default().fg(lc).add_modifier(Modifier::BOLD)),
                Span::styled(&e.message, Style::default().fg(color::TEXT)),
            ]))
        }).collect();

        frame.render_widget(List::new(items).style(Style::default().bg(color::SURFACE0)).block(block), area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let binds = format!(
            "  {}  |  {}  |  {}  |  {}  |  {}",
            i18n::tr("keybinds.tab"), i18n::tr("keybinds.lang"),
            i18n::tr("keybinds.reset"), i18n::tr("keybinds.quit"),
            i18n::tr("press_ctrl_c"),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(binds, Style::default().fg(color::OVERLAY0))))
                .style(Style::default().bg(color::SURFACE0)),
            area,
        );
    }
}

// ===== Public API =====

pub async fn run_tui(
    cancel: Arc<std::sync::atomic::AtomicBool>,
    state_rx: watch::Receiver<Option<hb_ipc::GameStateData>>,
    status_rx: watch::Receiver<ConnectionStatus>,
    log_buf: LogBuffer,
) {
    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("TUI init failed (non-terminal?): {e}, running headless");
            return;
        }
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<KeyEvent>(64);

    let tx = event_tx.clone();
    std::thread::spawn(move || {
        loop {
            if crossterm::event::poll(Duration::from_millis(50)).ok() == Some(true) {
                if let Ok(Event::Key(key)) = crossterm::event::read() {
                    if tx.blocking_send(key).is_err() { break; }
                }
            }
        }
    });

    let mut app = TuiApp::new(state_rx, status_rx, log_buf.clone());
    app.pull_state();
    app.pull_connection();
    log_buf.push_str("info", "TUI started");

    let mut tick = tokio::time::interval(Duration::from_millis(33));

    loop {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) { break; }

        tokio::select! {
            Some(key) = event_rx.recv() => app.handle_key(key),
            _ = tick.tick() => {
                app.pull_state();
                app.pull_connection();
                let _ = terminal.draw(|frame| app.render(frame));
            }
        }
    }

    let _ = ratatui::try_restore();
    log_buf.push_str("info", "TUI stopped");
}
