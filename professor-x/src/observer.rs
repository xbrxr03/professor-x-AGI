use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Wrap};
use ratatui::{Frame, Terminal};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::memd::events::{AgentEvent, EventStore};
use crate::memd::MemoryManager;

const TICK_RATE: Duration = Duration::from_millis(750);

pub fn run_observer(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = ObserverApp::new(memory, events);
    let result = run_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

pub fn print_snapshot(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Result<()> {
    let snapshot = ObserverSnapshot::load(&memory, &events)?;
    println!("Professor X lab snapshot");
    println!(
        "  git: {} / {}{}",
        snapshot.git_branch,
        snapshot.git_commit,
        if snapshot.git_dirty {
            " dirty"
        } else {
            " clean"
        }
    );
    println!(
        "  scheduled jobs: {} active, {} paused",
        snapshot.active_jobs, snapshot.paused_jobs
    );
    println!(
        "  evolution: {} nodes, {} accepted, {} rejected",
        snapshot.evolution_nodes, snapshot.accepted_nodes, snapshot.rejected_nodes
    );
    if let Some(commit) = &snapshot.latest_evolution_commit {
        println!("  latest autonomous commit: {commit}");
    }
    println!(
        "  evolution artifacts: {} proposed, {} verified, {} accepted, {} rejected",
        snapshot.proposal_artifacts,
        snapshot.verification_artifacts,
        snapshot.accepted_artifacts,
        snapshot.rejected_artifacts
    );
    println!("  command artifacts: {}", snapshot.command_artifacts);
    let pass = snapshot
        .latest_pass_at_3
        .map(|v| format!("{v:.3}"))
        .unwrap_or_else(|| "not run".to_string());
    println!("  HIRO: {} rounds, pass@3 {pass}", snapshot.hiro_rounds);
    println!("  audit entries: {}", snapshot.audit_entries);
    println!("  task transcripts: {}", snapshot.transcript_count);
    println!("  events: {}", snapshot.total_events);
    println!("  recent events:");
    for event in snapshot.events.iter().rev().take(8).rev() {
        println!("  {}", format_event_line(event));
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut ObserverApp,
) -> Result<()> {
    app.refresh()?;
    loop {
        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break
                        }
                        KeyCode::Char('r') => app.refresh()?,
                        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                        KeyCode::PageUp => app.scroll_page_up(),
                        KeyCode::PageDown => app.scroll_page_down(),
                        KeyCode::Home => app.scroll_top(),
                        KeyCode::End => app.scroll_bottom(),
                        _ => {}
                    }
                }
            }
        }

        if app.last_refresh.elapsed() >= TICK_RATE {
            app.refresh()?;
        }
    }
    Ok(())
}

struct ObserverApp {
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    snapshot: ObserverSnapshot,
    selected_offset: usize,
    last_refresh: Instant,
    sparkline: Vec<u64>,
}

impl ObserverApp {
    fn new(memory: Arc<MemoryManager>, events: Arc<EventStore>) -> Self {
        Self {
            memory,
            events,
            snapshot: ObserverSnapshot::default(),
            selected_offset: usize::MAX,
            last_refresh: Instant::now() - TICK_RATE,
            sparkline: Vec::new(),
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.snapshot = ObserverSnapshot::load(&self.memory, &self.events)?;
        self.last_refresh = Instant::now();
        let event_count = self.snapshot.total_events.max(0) as u64;
        if self.sparkline.last().copied() != Some(event_count) {
            self.sparkline.push(event_count);
            if self.sparkline.len() > 40 {
                self.sparkline.remove(0);
            }
        }
        self.selected_offset = self
            .selected_offset
            .min(self.snapshot.events.len().saturating_sub(1));
        Ok(())
    }

    fn scroll_up(&mut self) {
        self.selected_offset = self.selected_offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.selected_offset =
            (self.selected_offset + 1).min(self.snapshot.events.len().saturating_sub(1));
    }

    fn scroll_page_up(&mut self) {
        self.selected_offset = self.selected_offset.saturating_sub(8);
    }

    fn scroll_page_down(&mut self) {
        self.selected_offset =
            (self.selected_offset + 8).min(self.snapshot.events.len().saturating_sub(1));
    }

    fn scroll_top(&mut self) {
        self.selected_offset = 0;
    }

    fn scroll_bottom(&mut self) {
        self.selected_offset = self.snapshot.events.len().saturating_sub(1);
    }
}

struct ObserverSnapshot {
    events: Vec<AgentEvent>,
    total_events: i64,
    active_jobs: i64,
    paused_jobs: i64,
    audit_entries: i64,
    transcript_count: i64,
    hiro_rounds: i64,
    latest_pass_at_3: Option<f64>,
    task_events: usize,
    tool_events: usize,
    policy_events: usize,
    evolution_events: usize,
    evolution_nodes: i64,
    accepted_nodes: i64,
    rejected_nodes: i64,
    proposal_artifacts: usize,
    verification_artifacts: usize,
    accepted_artifacts: usize,
    rejected_artifacts: usize,
    command_artifacts: usize,
    git_branch: String,
    git_commit: String,
    git_dirty: bool,
    latest_evolution_commit: Option<String>,
    latest_task: Option<AgentEvent>,
    latest_tool: Option<AgentEvent>,
    latest_policy: Option<AgentEvent>,
    latest_evolution: Option<AgentEvent>,
    latest_transcript: Option<AgentEvent>,
}

impl ObserverSnapshot {
    fn load(memory: &MemoryManager, events: &EventStore) -> Result<Self> {
        let recent = events.tail(120)?;
        let db = memory.db.lock().unwrap();
        let active_jobs: i64 = db.query_row(
            "SELECT COUNT(*) FROM cron_jobs WHERE enabled = 1 AND state = 'Scheduled'",
            [],
            |row| row.get(0),
        )?;
        let paused_jobs: i64 = db.query_row(
            "SELECT COUNT(*) FROM cron_jobs WHERE enabled = 0 OR state = 'Paused'",
            [],
            |row| row.get(0),
        )?;
        let audit_entries: i64 =
            db.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?;
        let transcript_count: i64 =
            db.query_row("SELECT COUNT(*) FROM task_transcripts", [], |row| {
                row.get(0)
            })?;
        let hiro_rounds: i64 =
            db.query_row("SELECT COUNT(*) FROM hiro_rounds", [], |row| row.get(0))?;
        let evolution_nodes: i64 =
            db.query_row("SELECT COUNT(*) FROM evolution_nodes", [], |row| row.get(0))?;
        let accepted_nodes: i64 = db.query_row(
            "SELECT COUNT(*) FROM evolution_nodes WHERE status = 'Accepted'",
            [],
            |row| row.get(0),
        )?;
        let rejected_nodes: i64 = db.query_row(
            "SELECT COUNT(*) FROM evolution_nodes WHERE status = 'Rejected'",
            [],
            |row| row.get(0),
        )?;
        let total_events: i64 =
            db.query_row("SELECT COUNT(*) FROM agent_events", [], |row| row.get(0))?;
        let latest_pass_at_3 = db
            .query_row(
                "SELECT pass_at_3 FROM hiro_rounds ORDER BY round DESC LIMIT 1",
                [],
                |row| row.get::<_, f64>(0),
            )
            .ok();
        drop(db);

        let mut snapshot = Self {
            events: recent,
            total_events,
            active_jobs,
            paused_jobs,
            audit_entries,
            transcript_count,
            hiro_rounds,
            evolution_nodes,
            accepted_nodes,
            rejected_nodes,
            latest_pass_at_3,
            ..Self::default()
        };

        let repo = repo_root();
        snapshot.git_branch = git_output(&repo, &["branch", "--show-current"])
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        snapshot.git_commit = git_output(&repo, &["rev-parse", "--short", "HEAD"])
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        snapshot.git_dirty = git_output(&repo, &["status", "--short"])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        snapshot.latest_evolution_commit =
            git_output(&repo, &["log", "--grep=^evolved:", "--format=%h %s", "-1"])
                .filter(|s| !s.is_empty());

        let artifact_root = evolution_artifact_root(&repo);
        snapshot.proposal_artifacts = count_json_files(&artifact_root.join("proposals"));
        snapshot.verification_artifacts = count_json_files(&artifact_root.join("verifications"));
        snapshot.accepted_artifacts = count_json_files(&artifact_root.join("accepted"));
        snapshot.rejected_artifacts = count_json_files(&artifact_root.join("rejections"));
        snapshot.command_artifacts = count_json_files(&generic_artifact_root(&repo).join("commands"));

        for event in &snapshot.events {
            if event.event_type.starts_with("task.") {
                snapshot.task_events += 1;
                snapshot.latest_task = Some(event.clone());
            } else if event.event_type.starts_with("tool.") {
                snapshot.tool_events += 1;
                snapshot.latest_tool = Some(event.clone());
            } else if event.event_type.starts_with("policy.") {
                snapshot.policy_events += 1;
                snapshot.latest_policy = Some(event.clone());
            } else if event.event_type.starts_with("evolution.") {
                snapshot.evolution_events += 1;
                snapshot.latest_evolution = Some(event.clone());
            } else if event.event_type.starts_with("transcript.") {
                snapshot.latest_transcript = Some(event.clone());
            }
        }

        Ok(snapshot)
    }
}

fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

fn evolution_artifact_root(repo: &Path) -> PathBuf {
    let nested = repo.join("professor-x/artifacts/evolution");
    if nested.exists() {
        nested
    } else {
        repo.join("artifacts/evolution")
    }
}

fn generic_artifact_root(repo: &Path) -> PathBuf {
    let nested = repo.join("professor-x/artifacts");
    if nested.exists() {
        nested
    } else {
        repo.join("artifacts")
    }
}

fn git_output(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn count_json_files(root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_json_files(&path)
            } else if path.extension().is_some_and(|ext| ext == "json") {
                1
            } else {
                0
            }
        })
        .sum()
}

impl Default for ObserverSnapshot {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            total_events: 0,
            active_jobs: 0,
            paused_jobs: 0,
            audit_entries: 0,
            transcript_count: 0,
            hiro_rounds: 0,
            latest_pass_at_3: None,
            task_events: 0,
            tool_events: 0,
            policy_events: 0,
            evolution_events: 0,
            evolution_nodes: 0,
            accepted_nodes: 0,
            rejected_nodes: 0,
            proposal_artifacts: 0,
            verification_artifacts: 0,
            accepted_artifacts: 0,
            rejected_artifacts: 0,
            command_artifacts: 0,
            git_branch: "unknown".to_string(),
            git_commit: "unknown".to_string(),
            git_dirty: false,
            latest_evolution_commit: None,
            latest_task: None,
            latest_tool: None,
            latest_policy: None,
            latest_evolution: None,
            latest_transcript: None,
        }
    }
}

fn draw(frame: &mut Frame, app: &ObserverApp) {
    let area = frame.area();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, root[0], app);
    draw_body(frame, root[1], app);
    draw_footer(frame, root[2]);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let title = Line::from(vec![
        Span::styled(
            "PROFESSOR X",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  autonomous research harness observer  "),
        Span::styled(
            "local-first / audited / HIRO-aware",
            Style::default().fg(Color::Gray),
        ),
    ]);
    let subtitle = Line::from(vec![
        Span::styled("events ", Style::default().fg(Color::Gray)),
        Span::styled(
            app.snapshot.total_events.to_string(),
            Style::default().fg(Color::Green),
        ),
        Span::raw("   "),
        Span::styled("jobs ", Style::default().fg(Color::Gray)),
        Span::styled(
            app.snapshot.active_jobs.to_string(),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" active   "),
        Span::styled("audit ", Style::default().fg(Color::Gray)),
        Span::styled(
            app.snapshot.audit_entries.to_string(),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw(" entries   "),
        Span::styled("transcripts ", Style::default().fg(Color::Gray)),
        Span::styled(
            app.snapshot.transcript_count.to_string(),
            Style::default().fg(Color::Green),
        ),
        Span::raw("   "),
        Span::styled("git ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!(
                "{}/{}{}",
                app.snapshot.git_branch,
                app.snapshot.git_commit,
                if app.snapshot.git_dirty { "*" } else { "" }
            ),
            if app.snapshot.git_dirty {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            },
        ),
    ]);
    frame.render_widget(
        Paragraph::new(vec![title, subtitle])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
        area,
    );
}

fn draw_body(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(8),
        ])
        .split(columns[0]);
    draw_status(frame, left[0], app);
    draw_activity(frame, left[1], app);
    draw_science(frame, left[2], app);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(columns[1]);
    draw_timeline(frame, right[0], app);
    draw_event_detail(frame, right[1], app);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let pass = app
        .snapshot
        .latest_pass_at_3
        .map(|v| format!("{:.3}", v))
        .unwrap_or_else(|| "not run".to_string());
    let lines = vec![
        Line::from(vec![
            Span::styled("Scheduler   ", label()),
            Span::raw(format!(
                "{} active / {} paused",
                app.snapshot.active_jobs, app.snapshot.paused_jobs
            )),
        ]),
        Line::from(vec![
            Span::styled("HIRO        ", label()),
            Span::raw(format!(
                "{} rounds / pass@3 {pass}",
                app.snapshot.hiro_rounds
            )),
        ]),
        Line::from(vec![
            Span::styled("Evolution   ", label()),
            Span::raw(format!(
                "{} nodes / {} accepted / {} rejected",
                app.snapshot.evolution_nodes,
                app.snapshot.accepted_nodes,
                app.snapshot.rejected_nodes
            )),
        ]),
        Line::from(vec![
            Span::styled("Audit       ", label()),
            Span::raw(format!("{} entries", app.snapshot.audit_entries)),
        ]),
        Line::from(vec![
            Span::styled("Transcripts ", label()),
            Span::raw(format!("{} tasks", app.snapshot.transcript_count)),
        ]),
        Line::from(vec![
            Span::styled("Git         ", label()),
            Span::raw(format!(
                "{} / {}{}",
                app.snapshot.git_branch,
                app.snapshot.git_commit,
                if app.snapshot.git_dirty {
                    " dirty"
                } else {
                    " clean"
                }
            )),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("system state"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_activity(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let lines = vec![
        latest_line("task", &app.snapshot.latest_task),
        latest_line("tool", &app.snapshot.latest_tool),
        latest_line("policy", &app.snapshot.latest_policy),
        latest_line("trace", &app.snapshot.latest_transcript),
        latest_line("evolve", &app.snapshot.latest_evolution),
        Line::from(vec![
            Span::styled("commit  ", label()),
            Span::raw(
                app.snapshot
                    .latest_evolution_commit
                    .clone()
                    .unwrap_or_else(|| "waiting".to_string()),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("current work"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_science(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(area);
    let total = app.snapshot.events.len().max(1) as f64;
    let task_ratio = app.snapshot.task_events as f64 / total;
    frame.render_widget(
        Gauge::default()
            .block(panel("recent signal"))
            .gauge_style(Style::default().fg(Color::Cyan))
            .label(format!(
                "task {}  tool {}  policy {}  evolution {}",
                app.snapshot.task_events,
                app.snapshot.tool_events,
                app.snapshot.policy_events,
                app.snapshot.evolution_events,
            ))
            .ratio(task_ratio.clamp(0.0, 1.0)),
        chunks[0],
    );
    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Green))
            .data(&app.sparkline),
        chunks[1],
    );
    let note = Paragraph::new(vec![
        Line::from(format!(
            "Evolution artifacts: {} proposed / {} verified / {} accepted / {} rejected",
            app.snapshot.proposal_artifacts,
            app.snapshot.verification_artifacts,
            app.snapshot.accepted_artifacts,
            app.snapshot.rejected_artifacts,
        )),
        Line::from(format!(
            "Command output artifacts: {}",
            app.snapshot.command_artifacts,
        )),
        Line::from(
            "Run --lab --run-now for daemon plus observer; --observe follows an existing run.",
        ),
    ])
    .style(Style::default().fg(Color::Gray))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(note, chunks[2]);
}

fn draw_timeline(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let visible_events = visible_events(app, area.height.saturating_sub(2) as usize);
    let items = visible_events
        .iter()
        .enumerate()
        .map(|(idx, event)| {
            let absolute_idx = app
                .snapshot
                .events
                .len()
                .saturating_sub(visible_events.len())
                + idx;
            let marker = if absolute_idx == app.selected_offset {
                ">"
            } else {
                " "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::raw(format!(" #{:05} ", event.id)),
                Span::styled(
                    event.timestamp.format("%H:%M:%S").to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<22}", event.event_type),
                    event_style(&event.event_type),
                ),
                Span::raw("  "),
                Span::raw(truncate(
                    &event.summary,
                    area.width.saturating_sub(46) as usize,
                )),
            ]))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items)
            .block(panel("live trace"))
            .style(Style::default().fg(Color::White)),
        area,
    );
}

fn draw_event_detail(frame: &mut Frame, area: Rect, app: &ObserverApp) {
    let selected = app.snapshot.events.get(app.selected_offset);
    let lines = match selected {
        Some(event) => vec![
            Line::from(vec![
                Span::styled("id      ", label()),
                Span::raw(format!("#{}", event.id)),
                Span::raw("   "),
                Span::styled("time ", label()),
                Span::raw(event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()),
            ]),
            Line::from(vec![
                Span::styled("type    ", label()),
                Span::styled(event.event_type.clone(), event_style(&event.event_type)),
            ]),
            Line::from(vec![
                Span::styled("task    ", label()),
                Span::raw(event.task_id.clone().unwrap_or_else(|| "-".to_string())),
            ]),
            Line::from(vec![
                Span::styled("session ", label()),
                Span::raw(event.session_id.clone().unwrap_or_else(|| "-".to_string())),
            ]),
            Line::from(vec![
                Span::styled("summary ", label()),
                Span::raw(event.summary.clone()),
            ]),
            Line::from(vec![
                Span::styled("payload ", label()),
                Span::raw(truncate(
                    &event.payload.to_string(),
                    area.width.saturating_mul(3) as usize,
                )),
            ]),
        ],
        None => vec![Line::from("No events recorded yet.")],
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("selected event"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn visible_events(app: &ObserverApp, height: usize) -> &[AgentEvent] {
    if app.snapshot.events.len() <= height {
        return &app.snapshot.events;
    }
    let end = (app.selected_offset + 1)
        .max(height)
        .min(app.snapshot.events.len());
    let start = end.saturating_sub(height);
    &app.snapshot.events[start..end]
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("q/esc", hotkey()),
        Span::raw(" quit   "),
        Span::styled("r", hotkey()),
        Span::raw(" refresh   "),
        Span::styled("j/k", hotkey()),
        Span::raw(" scroll   "),
        Span::styled("source", label()),
        Span::raw(" ~/.professor-x/state.db + professor-x/artifacts/events/*.jsonl"),
    ]);
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn latest_line(label_text: &str, event: &Option<AgentEvent>) -> Line<'static> {
    match event {
        Some(event) => Line::from(vec![
            Span::styled(format!("{label_text:<8}"), label()),
            Span::styled(
                format!("{:<20}", event.event_type),
                event_style(&event.event_type),
            ),
            Span::raw(truncate(&event.summary, 64)),
        ]),
        None => Line::from(vec![
            Span::styled(format!("{label_text:<8}"), label()),
            Span::styled("waiting", Style::default().fg(Color::DarkGray)),
        ]),
    }
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn label() -> Style {
    Style::default()
        .fg(Color::Gray)
        .add_modifier(Modifier::BOLD)
}

fn hotkey() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn event_style(event_type: &str) -> Style {
    let color = if event_type.starts_with("task.") {
        Color::Cyan
    } else if event_type.starts_with("tool.") {
        Color::Green
    } else if event_type.starts_with("policy.denied") || event_type.ends_with(".error") {
        Color::Red
    } else if event_type.starts_with("policy.") {
        Color::Yellow
    } else if event_type.starts_with("evolution.") {
        Color::Magenta
    } else if event_type.starts_with("hiro.") {
        Color::Blue
    } else {
        Color::White
    };
    Style::default().fg(color)
}

fn truncate(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn format_event_line(event: &AgentEvent) -> String {
    let task = event
        .task_id
        .as_ref()
        .map(|id| format!(" task={}", &id[..8.min(id.len())]))
        .unwrap_or_default();
    let session = event
        .session_id
        .as_ref()
        .map(|id| format!(" session={}", &id[..8.min(id.len())]))
        .unwrap_or_default();
    format!(
        "#{:05} {} {:<22}{}{} {}",
        event.id,
        event.timestamp.format("%Y-%m-%d %H:%M:%S"),
        event.event_type,
        task,
        session,
        event.summary
    )
}
