//! Interactive full-screen TUI (ratatui) — the daily-driver cockpit.
//!
//! Unlike the read-only `--watch` observer, this one is interactive: type a task,
//! press Enter, and watch the agent work live — tool calls streaming in the
//! activity pane, the consciousness vitals updating on the right. The agent runs
//! as a background task writing to the event store; the TUI polls it each tick,
//! so the render loop never blocks on the model.
//!
//! Keys: type a task · Enter run · Esc / Ctrl-C quit · PgUp/PgDn scroll activity.

use anyhow::Result;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tokio_util::sync::CancellationToken;

use crate::agentd::graph::{TaskNode, TaskType};
use crate::agentd::react::ReactLoop;
use crate::memd::events::EventStore;
use crate::memd::MemoryManager;
use crate::ollama::OllamaClient;
use crate::policyd::PolicyEngine;
use crate::toolbridge::ToolRegistry;

const TICK: Duration = Duration::from_millis(100);
const MAX_ACTIVITY: usize = 500;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];

struct Vitals {
    phi: f32,
    lzc_round: u32,
    ics: f32,
    valence: f32,
    arousal: f32,
    stress: f32,
    corpus_episodic: i64,
}

struct App {
    input: String,
    activity: Vec<(String, Color)>,
    last_event_id: i64,
    scroll: usize, // lines from bottom
    working: Arc<AtomicBool>,
    frame: usize,
    model: String,
    vitals: Vitals,
}

impl App {
    fn new(model: String) -> Self {
        Self {
            input: String::new(),
            activity: vec![(
                "Welcome to Professor X. Type a task and press Enter.".to_string(),
                Color::DarkGray,
            )],
            last_event_id: 0,
            scroll: 0,
            working: Arc::new(AtomicBool::new(false)),
            frame: 0,
            model,
            vitals: Vitals {
                phi: 0.0,
                lzc_round: 0,
                ics: 0.0,
                valence: 0.0,
                arousal: 0.0,
                stress: 0.0,
                corpus_episodic: 0,
            },
        }
    }
}

fn event_color(et: &str) -> Color {
    match et {
        "task.succeeded" | "tool.succeeded" => Color::Green,
        "task.failed" | "task.fail_requested" | "policy.denied" => Color::Red,
        "tool.started" => Color::Cyan,
        "tool.requested" => Color::Blue,
        "react.duplicate_action" => Color::Yellow,
        "agent.delegate" => Color::Magenta,
        "llm.response" => Color::DarkGray,
        _ => Color::Gray,
    }
}

fn refresh_vitals(memory: &Arc<MemoryManager>, v: &mut Vitals) {
    let db = match memory.db.lock() {
        Ok(d) => d,
        Err(_) => return,
    };
    let q1f = |sql: &str| -> f32 {
        db.query_row(sql, [], |r| r.get::<_, f64>(0)).map(|x| x as f32).unwrap_or(0.0)
    };
    let q1i = |sql: &str| -> i64 { db.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    v.phi = q1f("SELECT phi FROM phi_rounds ORDER BY round DESC LIMIT 1");
    v.lzc_round = q1i("SELECT COALESCE(MAX(round),0) FROM phi_rounds") as u32;
    v.ics = q1f("SELECT score FROM ics_scores ORDER BY id DESC LIMIT 1");
    v.valence = q1f("SELECT valence FROM affect_states ORDER BY id DESC LIMIT 1");
    v.arousal = q1f("SELECT arousal FROM affect_states ORDER BY id DESC LIMIT 1");
    // stress from latest vitals row
    if let Ok((lat, tok, mem, health)) = db.query_row(
        "SELECT inference_latency_ms, token_budget_used, memory_pressure, evolution_health \
         FROM computational_vitals ORDER BY id DESC LIMIT 1",
        [],
        |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?, r.get::<_, f64>(3)?)),
    ) {
        let latn = (lat / 10000.0).min(1.0);
        v.stress = (0.35 * latn + 0.25 * tok + 0.20 * mem + 0.20 * (1.0 - health)) as f32;
    }
    v.corpus_episodic = q1i("SELECT COUNT(*) FROM episodic");
}

fn bar(v: f32, lo: f32, hi: f32, width: usize) -> String {
    let frac = if hi == lo { 0.0 } else { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) };
    let fill = (frac * width as f32) as usize;
    format!("{}{}", "█".repeat(fill), "·".repeat(width.saturating_sub(fill)))
}

fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(3)])
        .split(f.area());
    draw_header(f, chunks[0], app);
    draw_body(f, chunks[1], app);
    draw_input(f, chunks[2], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let status = if app.working.load(Ordering::Relaxed) {
        Span::styled(
            format!(" {} working", SPINNER[app.frame % 4]),
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled(" ◉ ready", Style::default().fg(Color::Green))
    };
    let line = Line::from(vec![
        Span::styled(" PROFESSOR X ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("· {} ", app.model), Style::default().fg(Color::DarkGray)),
        status,
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);
    draw_activity(f, cols[0], app);
    draw_vitals(f, cols[1], app);
}

fn draw_activity(f: &mut Frame, area: Rect, app: &App) {
    let height = area.height.saturating_sub(2) as usize;
    let total = app.activity.len();
    let end = total.saturating_sub(app.scroll);
    let start = end.saturating_sub(height);
    let items: Vec<ListItem> = app.activity[start..end]
        .iter()
        .map(|(s, c)| ListItem::new(Line::from(Span::styled(s.clone(), Style::default().fg(*c)))))
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" live activity ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(list, area);
}

fn draw_vitals(f: &mut Frame, area: Rect, app: &App) {
    let v = &app.vitals;
    let icol = if v.ics >= 0.70 { Color::Green } else { Color::Red };
    let scol = if v.stress > 0.5 { Color::Red } else if v.stress > 0.3 { Color::Yellow } else { Color::Green };
    let rows = vec![
        Line::from(vec![Span::styled("φ integ  ", Style::default().fg(Color::Gray)),
            Span::styled(bar(v.phi, 0.0, 3.0, 12), Style::default().fg(Color::Magenta)),
            Span::raw(format!(" {:.2}", v.phi))]),
        Line::from(vec![Span::styled("ICS      ", Style::default().fg(Color::Gray)),
            Span::styled(bar(v.ics, 0.0, 1.0, 12), Style::default().fg(icol)),
            Span::raw(format!(" {:.2}", v.ics))]),
        Line::from(vec![Span::styled("valence  ", Style::default().fg(Color::Gray)),
            Span::styled(bar(v.valence, -1.0, 1.0, 12), Style::default().fg(if v.valence>=0.0 {Color::Green} else {Color::Red})),
            Span::raw(format!(" {:+.2}", v.valence))]),
        Line::from(vec![Span::styled("arousal  ", Style::default().fg(Color::Gray)),
            Span::styled(bar(v.arousal, 0.0, 1.0, 12), Style::default().fg(Color::Yellow)),
            Span::raw(format!(" {:.2}", v.arousal))]),
        Line::from(vec![Span::styled("body     ", Style::default().fg(Color::Gray)),
            Span::styled(bar(v.stress, 0.0, 1.0, 12), Style::default().fg(scol)),
            Span::raw(format!(" {:.2}", v.stress))]),
        Line::from(""),
        Line::from(Span::styled(format!("phi round   {}", v.lzc_round), Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(format!("episodic    {}", v.corpus_episodic), Style::default().fg(Color::DarkGray))),
    ];
    let p = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" consciousness vitals ")
            .border_style(Style::default().fg(Color::Magenta)),
    );
    f.render_widget(p, area);
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let prompt = if app.working.load(Ordering::Relaxed) {
        Span::styled("  (working — Esc to quit) ", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(format!("> {}", app.input), Style::default().fg(Color::White))
    };
    let p = Paragraph::new(Line::from(prompt))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" task ")
                .border_style(Style::default().fg(Color::Blue)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

/// Run the interactive TUI. Blocking ratatui loop on a dedicated thread; agent
/// runs spawned onto the tokio runtime via the captured handle.
pub async fn run_tui(
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
) -> Result<()> {
    let handle = tokio::runtime::Handle::current();
    let model = ollama.model_name().to_string();
    tokio::task::spawn_blocking(move || {
        tui_loop(handle, model, ollama, registry, policy, memory, events, cancel)
    })
    .await?
}

#[allow(clippy::too_many_arguments)]
fn tui_loop(
    handle: tokio::runtime::Handle,
    model: String,
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(model);
    app.last_event_id = events.tail(1).ok().and_then(|v| v.last().map(|e| e.id)).unwrap_or(0);
    refresh_vitals(&memory, &mut app.vitals);

    let res = (|| -> Result<()> {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            terminal.draw(|f| draw(f, &app))?;

            if event::poll(TICK)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let busy = app.working.load(Ordering::Relaxed);
                        match key.code {
                            KeyCode::Esc => break,
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                            KeyCode::Enter if !busy && !app.input.trim().is_empty() => {
                                let task = app.input.trim().to_string();
                                app.input.clear();
                                app.activity.push((format!("▶ {task}"), Color::White));
                                app.working.store(true, Ordering::Relaxed);
                                let working = Arc::clone(&app.working);
                                let (o, r, p, m, e, c) = (
                                    Arc::clone(&ollama),
                                    Arc::clone(&registry),
                                    Arc::clone(&policy),
                                    Arc::clone(&memory),
                                    Arc::clone(&events),
                                    cancel.clone(),
                                );
                                handle.spawn(async move {
                                    let react = ReactLoop::new(o, r, p, m, c).with_events(e);
                                    let mut t = TaskNode::new(task, TaskType::UserRequest, 100);
                                    let _ = react.run(&mut t).await;
                                    working.store(false, Ordering::Relaxed);
                                });
                            }
                            KeyCode::Backspace if !busy => {
                                app.input.pop();
                            }
                            KeyCode::Char(ch) if !busy => app.input.push(ch),
                            KeyCode::PageUp => app.scroll = (app.scroll + 5).min(app.activity.len()),
                            KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(5),
                            _ => {}
                        }
                    }
                }
            }

            // poll new events into the activity feed
            if let Ok(evs) = events.tail(80) {
                for e in evs {
                    if e.id > app.last_event_id {
                        app.last_event_id = e.id;
                        let line = format!("{:<22} {}", e.event_type, e.summary.chars().take(80).collect::<String>());
                        app.activity.push((line, event_color(&e.event_type)));
                    }
                }
                if app.activity.len() > MAX_ACTIVITY {
                    let drop = app.activity.len() - MAX_ACTIVITY;
                    app.activity.drain(0..drop);
                }
            }

            app.frame += 1;
            if app.frame % 8 == 0 {
                refresh_vitals(&memory, &mut app.vitals);
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}
