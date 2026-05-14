use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

const COMMANDS: &[(&str, &str)] = &[
    ("/map", "Карта знаний"),
    ("/gaps", "Пробелы в разведке"),
    ("/scout", "Авто-разведка"),
    ("/assimilate", "Полная ассимиляция"),
    ("/code", "GOD MODE генерация"),
    ("/research", "Исследовать конкурента"),
    ("/darknet", "Исследовать угрозу"),
    ("/dashboard", "Дашборд"),
    ("/rollback", "Откат к снапшоту"),
    ("/save", "Сохранить сессию"),
    ("/load", "Загрузить сессию"),
    ("/help", "Помощь"),
];

struct DashboardApp {
    command_list_state: ListState,
    animation_phase: f64,
    osint_density: f64,
    darknet_density: f64,
    shield_active: bool,
    shield_power: f64,
    last_frame: Instant,
}

impl DashboardApp {
    fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            command_list_state: state,
            animation_phase: 0.0,
            osint_density: 0.5,
            darknet_density: 0.5,
            shield_active: false,
            shield_power: 0.0,
            last_frame: Instant::now(),
        }
    }

    fn next_command(&mut self) {
        let i = match self.command_list_state.selected() {
            Some(i) => if i >= COMMANDS.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.command_list_state.select(Some(i));
    }

    fn previous_command(&mut self) {
        let i = match self.command_list_state.selected() {
            Some(i) => if i == 0 { COMMANDS.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.command_list_state.select(Some(i));
    }

    fn get_selected_command(&self) -> &str {
        match self.command_list_state.selected() {
            Some(i) => COMMANDS[i].0,
            None => "",
        }
    }

    fn tick(&mut self, is_assimilating: bool) {
        let dt = self.last_frame.elapsed().as_secs_f64();
        self.last_frame = Instant::now();

        let speed = if is_assimilating { 4.0 } else { 1.0 };
        self.animation_phase += dt * speed;
        if self.animation_phase > 2.0 * std::f64::consts::PI {
            self.animation_phase -= 2.0 * std::f64::consts::PI;
        }

        self.osint_density = (self.osint_density + dt * 0.1 * speed).min(1.0);
        self.darknet_density = (self.darknet_density + dt * 0.08 * speed).min(1.0);

        if self.osint_density > 0.7 && self.darknet_density > 0.7 {
            self.shield_active = true;
            self.shield_power = (self.shield_power + dt * 0.5).min(1.0);
        } else {
            self.shield_active = false;
            self.shield_power = (self.shield_power - dt * 0.3).max(0.0);
        }
    }
}

pub fn run_dashboard() -> Result<String, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let start_time = Instant::now();
    let mut app = DashboardApp::new();
    let tick_rate = Duration::from_millis(50);
    let result = run_app(&mut terminal, &mut app, start_time, tick_rate);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut DashboardApp,
    start_time: Instant,
    tick_rate: Duration,
) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        app.tick(false);

        terminal.draw(|f| ui(f, app, start_time))?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok("exit".to_string()),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_command(),
                    KeyCode::Down | KeyCode::Char('j') => app.next_command(),
                    KeyCode::Enter => return Ok(app.get_selected_command().to_string()),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut DashboardApp, start_time: Instant) {
    let area = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = if app.shield_active {
        Paragraph::new("🛡️ AEGIS COMMAND CENTER v8.0 — SHIELD ACTIVE 🛡️")
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Paragraph::new("🧬 AEGIS COMMAND CENTER v8.0")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    };
    let title = title
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)));
    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(75),
        ])
        .split(chunks[1]);

    render_command_menu(f, app, main_chunks[0]);

    let dash_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(6),
        ])
        .split(main_chunks[1]);

    render_stats(f, app, dash_chunks[0]);
    render_dna_helix(f, app, dash_chunks[1]);
    render_log(f, dash_chunks[2]);

    let status = format!(
        " Oracle: {} | Sentinel: 1 | Uptime: {}s | Shield: {} | Q — выход, ↑↓ — навигация, Enter — выбрать",
        if app.shield_active { "🟢 SHIELD" } else { "🟢" },
        start_time.elapsed().as_secs(),
        if app.shield_active { format!("{:.0}%", app.shield_power * 100.0) } else { "OFF".into() },
    );
    let status_bar = Paragraph::new(status)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status_bar, chunks[2]);
}

fn render_command_menu(f: &mut Frame, app: &mut DashboardApp, area: Rect) {
    let items: Vec<ListItem> = COMMANDS
        .iter()
        .map(|(cmd, desc)| ListItem::new(format!(" {}  {}", cmd, desc)))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Команды "))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.command_list_state);
}

fn render_stats(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let stats = vec![
        Line::from(vec![
            Span::styled("🛡️ Заблокировано угроз: ", Style::default().fg(Color::Green)),
            Span::styled("0", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("⚠️  Уровень риска: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:.1}%", (1.0 - app.shield_power) * 100.0), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("📚 База Знаний: ", Style::default().fg(Color::Blue)),
            Span::styled(
                format!("OSINT: {:.0}% | DarkNet: {:.0}%", app.osint_density * 100.0, app.darknet_density * 100.0),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(stats).block(Block::default().borders(Borders::ALL).title(" Статистика ")),
        area,
    );
}

fn render_dna_helix(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let width = area.width as usize;
    let height = area.height as usize;

    let mut lines: Vec<Line> = Vec::new();

    for row in 0..height.min(20) {
        let t = row as f64 / height as f64;
        let phase = app.animation_phase;
        let half_width = (width / 2) as f64;

        let left_x = half_width - (t * std::f64::consts::PI * 2.0 + phase).sin() * half_width * 0.8;
        let right_x = half_width + (t * std::f64::consts::PI * 2.0 + phase + std::f64::consts::PI).sin() * half_width * 0.8;

        let mut spans: Vec<Span> = Vec::new();

        for col in 0..width {
            let col_f = col as f64;
            let dist_left = (col_f - left_x).abs();
            let dist_right = (col_f - right_x).abs();

            let left_radius = 1.5 + app.osint_density * 2.0;
            let right_radius = 1.5 + app.darknet_density * 2.0;

            if dist_left < left_radius {
                let brightness = (1.0 - dist_left / left_radius) as f32;
                let color = if app.shield_active {
                    Color::Rgb(
                        (0.0 * brightness * 255.0) as u8,
                        (1.0 * brightness * 255.0) as u8,
                        (0.5 * brightness * 255.0) as u8,
                    )
                } else {
                    Color::Rgb(
                        0,
                        (0.7 * brightness * 255.0) as u8,
                        (1.0 * brightness * 255.0) as u8,
                    )
                };
                let symbol = if brightness > 0.7 { "●" } else { "·" };
                spans.push(Span::styled(symbol, Style::default().fg(color)));
            } else if dist_right < right_radius {
                let brightness = (1.0 - dist_right / right_radius) as f32;
                let color = if app.shield_active {
                    Color::Rgb(
                        (1.0 * brightness * 255.0) as u8,
                        0,
                        (0.8 * brightness * 255.0) as u8,
                    )
                } else {
                    Color::Rgb(
                        (1.0 * brightness * 255.0) as u8,
                        0,
                        (1.0 * brightness * 255.0) as u8,
                    )
                };
                let symbol = if brightness > 0.7 { "●" } else { "·" };
                spans.push(Span::styled(symbol, Style::default().fg(color)));
            } else {
                spans.push(Span::styled(" ", Style::default()));
            }
        }

        lines.push(Line::from(spans));
    }

    let block = if app.shield_active {
        let shield_color = Color::Rgb(
            (0.3 * app.shield_power * 255.0) as u8,
            (1.0 * app.shield_power * 255.0) as u8,
            (0.5 * app.shield_power * 255.0) as u8,
        );
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" 🧬 DNA SHIELD — {:.0}% 🧬 ", app.shield_power * 100.0))
            .style(Style::default().fg(shield_color))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .title(" 🧬 DNA Helix — OSINT / DarkNet 🧬 ")
            .style(Style::default().fg(Color::Cyan))
    };

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_log(f: &mut Frame, area: Rect) {
    let log = vec![
        Line::from(vec![
            Span::styled("[v8.0] ", Style::default().fg(Color::Gray)),
            Span::styled("ДНК-визуализация активна", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("[v8.0] ", Style::default().fg(Color::Gray)),
            Span::styled("Streaming reasoning: готов", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("[v8.0] ", Style::default().fg(Color::Gray)),
            Span::styled("Git snapshots: включены", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("[v8.0] ", Style::default().fg(Color::Gray)),
            Span::styled("Двухцветная защита: OSINT + DarkNet", Style::default().fg(Color::Magenta)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(log).block(Block::default().borders(Borders::ALL).title(" Лог событий ")),
        area,
    );
}