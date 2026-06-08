//! Interactive coding-agent TUI (ratatui) — `profx --tui`.
//!
//! Conversation-centric, like Claude Code / OpenCode: you type a request, the
//! agent talks back and its actions (reading, editing, running commands) read as
//! a clean transcript. The consciousness vitals are demoted to a footer line
//! (Tab expands them into a side panel) so the default view feels like a coding
//! assistant, not a monitoring dashboard.
//!
//! Keys: type · Enter run · Tab toggle vitals · PgUp/PgDn scroll · Esc/Ctrl-C quit.

use anyhow::Result;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph};
use ratatui::{Frame, Terminal};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::agentd::graph::{TaskNode, TaskType};
use crate::agentd::react::ReactLoop;
use crate::memd::autonomy_queue::{AutonomyQueueItem, AutonomyQueueStore};
use crate::memd::MemoryManager;
use crate::memd::events::EventStore;
use crate::ollama::OllamaClient;
use crate::policyd::PolicyEngine;
use crate::toolbridge::ToolRegistry;

const TICK: Duration = Duration::from_millis(100);
const MAX_LINES: usize = 1200;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];
const TUI_QUEUE_STEP_COMMAND: &str = "cargo run -- --prof-x-step-live 1";

// One-Dark-ish palette.
const ACCENT: Color = Color::Rgb(198, 120, 221);
const CYAN: Color = Color::Rgb(86, 182, 194);
const GREEN: Color = Color::Rgb(152, 195, 121);
const RED: Color = Color::Rgb(224, 108, 117);
const YELLOW: Color = Color::Rgb(229, 192, 123);
const DIM: Color = Color::Rgb(92, 99, 112);
const FG: Color = Color::Rgb(200, 204, 212);

struct Vitals {
    phi: f32,
    ics: f32,
    valence: f32,
    arousal: f32,
    stress: f32,
    phi_rounds: u32,
    episodic: i64,
}

struct App {
    input: String,
    lines: Vec<Line<'static>>,
    last_event_id: i64,
    scroll: usize,
    show_vitals: bool,
    working: Arc<AtomicBool>,
    frame: usize,
    model: String,
    vitals: Vitals,
    queue: QueueSignal,
}

#[derive(Clone, Debug, Default)]
struct QueueSignal {
    pending: i64,
    latest_status: String,
    latest_id: String,
    latest_goal: String,
    latest_command: String,
}

impl App {
    fn new(model: String) -> Self {
        let lines = vec![
            styled("Just tell me what you want done. For example:", FG),
            styled("   what does @src/main.rs do?", CYAN),
            styled(
                "   create a script that renames every .txt here to .md",
                CYAN,
            ),
            styled("   find every TODO in the codebase", CYAN),
            styled("   run the tests and tell me what's failing", CYAN),
            styled("@path pulls a file into context · Tab toggles vitals", DIM),
            Line::from(""),
        ];
        Self {
            input: String::new(),
            lines,
            last_event_id: 0,
            scroll: 0,
            show_vitals: false,
            working: Arc::new(AtomicBool::new(false)),
            frame: 0,
            model,
            vitals: Vitals {
                phi: 0.0,
                ics: 0.0,
                valence: 0.0,
                arousal: 0.0,
                stress: 0.0,
                phi_rounds: 0,
                episodic: 0,
            },
            queue: QueueSignal::default(),
        }
    }

    fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
        if self.lines.len() > MAX_LINES {
            let drop = self.lines.len() - MAX_LINES;
            self.lines.drain(0..drop);
        }
    }
}

fn styled(s: impl Into<String>, c: Color) -> Line<'static> {
    Line::from(Span::styled(s.into(), Style::default().fg(c)))
}

fn s(text: &Value, key: &str) -> String {
    text.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Translate one event into a transcript line — the coding-agent phrasing.
/// Returns None for noise (policy checks, requests, non-write completions).
fn event_to_line(event_type: &str, summary: &str, payload: &Value) -> Option<Line<'static>> {
    match event_type {
        // the agent talking
        "llm.response" => {
            let t = s(payload, "preview");
            let t = t.trim();
            if t.is_empty() {
                None
            } else {
                Some(styled(format!("  {t}"), FG))
            }
        }
        // an action being taken — phrase it like a coding agent
        "tool.started" => {
            let tool = s(payload, "tool");
            let p = s(payload, "params_preview");
            let (icon, verb) = match tool.as_str() {
                "fs.read" => ("○", "read"),
                "fs.list" => ("○", "list"),
                "fs.search" => ("⌕", "search files"),
                "fs.write" => ("✎", "write"),
                "fs.replace" => ("✎", "edit"),
                "fs.delete" => ("✗", "delete"),
                "shell.restricted" | "shell.elevated" => ("$", ""),
                "web.search" => ("⌕", "search"),
                "web.fetch" => ("⌕", "fetch"),
                "patch.apply" => ("✎", "patch"),
                "repo.map" => ("◈", "map repo"),
                "memory.read" => ("◰", "recall"),
                "memory.write" => ("◰", "remember"),
                "ollama.complete" => ("…", "sub-query"),
                "agent.delegate" => ("⑂", "delegate"),
                "agent.critic" | "mirror.review" => ("◎", "self-review"),
                "tot.search" => ("⌥", "deliberate"),
                "meta.observe" => ("◔", "introspect"),
                "vision.analyze" => ("◭", "look at image"),
                "finish" | "done" | "fail" => return None,
                _ => ("·", tool.as_str()),
            };
            let detail = clean_params(&p);
            let text = if verb.is_empty() {
                format!("    {icon} {detail}")
            } else if detail.is_empty() {
                format!("    {icon} {verb}")
            } else {
                format!("    {icon} {verb} {detail}")
            };
            Some(styled(text, CYAN))
        }
        // file changes get the diff summary surfaced
        "tool.succeeded" => {
            let tool = s(payload, "tool");
            if matches!(tool.as_str(), "fs.write" | "fs.replace" | "patch.apply") {
                let out = s(payload, "output_preview");
                let first = out.lines().next().unwrap_or(&out).trim().to_string();
                if first.is_empty() {
                    None
                } else {
                    Some(styled(format!("      {first}"), GREEN))
                }
            } else {
                None
            }
        }
        "task.succeeded" => Some(styled("  ✓ done", GREEN)),
        "task.failed" | "task.fail_requested" => Some(styled("  ✗ couldn't complete that", RED)),
        "policy.denied" => Some(styled(format!("  ⛔ {summary}"), RED)),
        event if event.starts_with("autonomy.queue.") => {
            Some(styled(format!("  ◇ {}", queue_event_summary(event, summary, payload)), YELLOW))
        }
        "tui.command.started" => Some(styled(format!("  ▶ {summary}"), CYAN)),
        "tui.command.completed" => Some(styled(format!("  ✓ {summary}"), GREEN)),
        "tui.command.failed" => Some(styled(format!("  ✗ {summary}"), RED)),
        "react.duplicate_action" => None,
        _ => None,
    }
}

fn queue_event_summary(event_type: &str, summary: &str, payload: &Value) -> String {
    let queue = payload
        .get("queue_id")
        .and_then(|value| value.as_str())
        .map(short)
        .unwrap_or_else(|| "latest".to_string());
    match event_type {
        "autonomy.queue.enqueued" | "autonomy.queue.seeded" => {
            format!("queued work {queue}: {summary}")
        }
        "autonomy.queue.started" => format!("started queued work {queue}: {summary}"),
        "autonomy.queue.completed" => format!("completed queued work {queue}: {summary}"),
        "autonomy.queue.failed" => format!("queued work {queue} failed: {summary}"),
        "autonomy.queue.planned" => format!("planned queued work {queue}: {summary}"),
        _ => format!("{event_type} {queue}: {summary}"),
    }
}

fn clean_params(p: &str) -> String {
    // params_preview looks like "path=src/main.rs" / "command=cargo test" / "query=..."
    let v = p
        .trim()
        .trim_start_matches("path=")
        .trim_start_matches("command=")
        .trim_start_matches("query=")
        .trim_start_matches("url=")
        .trim_start_matches("goal=");
    v.chars().take(80).collect()
}

fn refresh_vitals(memory: &Arc<MemoryManager>, v: &mut Vitals) {
    let db = match memory.db.lock() {
        Ok(d) => d,
        Err(_) => return,
    };
    let qf = |sql: &str| -> f32 {
        db.query_row(sql, [], |r| r.get::<_, f64>(0))
            .map(|x| x as f32)
            .unwrap_or(0.0)
    };
    let qi = |sql: &str| -> i64 { db.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    v.phi = qf("SELECT phi FROM phi_rounds ORDER BY round DESC LIMIT 1");
    v.phi_rounds = qi("SELECT COUNT(*) FROM phi_rounds") as u32;
    v.ics = qf("SELECT score FROM ics_scores ORDER BY id DESC LIMIT 1");
    v.valence = qf("SELECT valence FROM affect_states ORDER BY id DESC LIMIT 1");
    v.arousal = qf("SELECT arousal FROM affect_states ORDER BY id DESC LIMIT 1");
    if let Ok((lat, tok, mem, health)) = db.query_row(
        "SELECT inference_latency_ms, token_budget_used, memory_pressure, evolution_health \
         FROM computational_vitals ORDER BY id DESC LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, f64>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        },
    ) {
        let latn = (lat / 10000.0).min(1.0);
        v.stress = (0.35 * latn + 0.25 * tok + 0.20 * mem + 0.20 * (1.0 - health)) as f32;
    }
    v.episodic = qi("SELECT COUNT(*) FROM episodic");
}

fn refresh_queue(memory: &Arc<MemoryManager>) -> QueueSignal {
    let store = AutonomyQueueStore::new(Arc::clone(&memory.db));
    let pending = store.count_pending().unwrap_or(0);
    let latest = store.recent(1).ok().and_then(|mut items| items.pop());
    latest
        .as_ref()
        .map(|item| queue_signal_from_item(item, pending))
        .unwrap_or_else(|| QueueSignal {
            pending,
            latest_status: "empty".to_string(),
            latest_id: String::new(),
            latest_goal: "no queued autonomous work".to_string(),
            latest_command: "cargo run -- --prof-x-enqueue \"goal\"".to_string(),
        })
}

fn queue_signal_from_item(item: &AutonomyQueueItem, pending: i64) -> QueueSignal {
    QueueSignal {
        pending,
        latest_status: item.status.clone(),
        latest_id: short(&item.id),
        latest_goal: item.goal.clone(),
        latest_command: queue_next_command(item),
    }
}

fn queue_next_command(item: &AutonomyQueueItem) -> String {
    let queue = short(&item.id);
    match item.status.as_str() {
        "pending" | "running" => TUI_QUEUE_STEP_COMMAND.to_string(),
        "passed" | "completed" => format!("cargo run -- --prof-x-queue-publish {queue}"),
        "failed" | "rejected" => format!("cargo run -- --prof-x-queue-review {queue}"),
        _ => format!("cargo run -- --prof-x-queue-review {queue}"),
    }
}

fn handle_tui_command(
    input: &str,
    memory: &Arc<MemoryManager>,
    events: &Arc<EventStore>,
) -> Result<Option<Vec<Line<'static>>>> {
    let input = input.trim();
    if input == "/help" {
        return Ok(Some(tui_help_lines()));
    }
    if input == "/queue" || input.starts_with("/queue ") {
        let limit = input
            .strip_prefix("/queue")
            .unwrap_or("")
            .trim()
            .parse::<usize>()
            .unwrap_or(5);
        return Ok(Some(tui_queue_lines(memory, limit)?));
    }
    if let Some(rest) = input.strip_prefix("/enqueue-commit") {
        return Ok(Some(tui_enqueue_lines(memory, events, rest, "commit", 5, 65)?));
    }
    if let Some(rest) = input.strip_prefix("/enqueue") {
        return Ok(Some(tui_enqueue_lines(memory, events, rest, "core", 4, 55)?));
    }
    if is_tui_step_command(input) {
        return Ok(Some(vec![
            styled("Queued work is advanced by the supervised runner.", DIM),
            styled(format!("   {TUI_QUEUE_STEP_COMMAND}"), CYAN),
        ]));
    }
    if input.starts_with('/') {
        return Ok(Some(vec![styled(
            format!("Unknown command: {input}. Try /help"),
            RED,
        )]));
    }
    Ok(None)
}

fn tui_help_lines() -> Vec<Line<'static>> {
    vec![
        styled("TUI commands", ACCENT),
        styled("   /queue [n]            show queued autonomous work", CYAN),
        styled("   /enqueue <goal>       queue a bounded core Prof X goal", CYAN),
        styled("   /enqueue-commit <goal> queue verified commit-capable work", CYAN),
        styled("   /step-live            run one supervised queue step", CYAN),
    ]
}

fn tui_queue_lines(memory: &Arc<MemoryManager>, limit: usize) -> Result<Vec<Line<'static>>> {
    let items = AutonomyQueueStore::new(Arc::clone(&memory.db)).recent(limit.clamp(1, 20))?;
    if items.is_empty() {
        return Ok(vec![styled(
            "Queue is empty. Use /enqueue <goal> or /enqueue-commit <goal>.",
            DIM,
        )]);
    }
    let mut lines = vec![styled("Autonomous queue", ACCENT)];
    for item in items {
        lines.push(styled(tui_queue_item_line(&item), CYAN));
        lines.push(styled(format!("      next {}", queue_next_command(&item)), DIM));
    }
    Ok(lines)
}

fn tui_enqueue_lines(
    memory: &Arc<MemoryManager>,
    events: &Arc<EventStore>,
    goal: &str,
    profile: &str,
    cycles: u32,
    priority: u8,
) -> Result<Vec<Line<'static>>> {
    let goal = sanitize_tui_goal(goal);
    if goal.is_empty() {
        return Ok(vec![styled(
            "Cannot enqueue an empty goal. Use /enqueue <goal>.",
            RED,
        )]);
    }
    let item = AutonomyQueueStore::new(Arc::clone(&memory.db)).enqueue(
        goal.clone(),
        "operator_run",
        profile,
        cycles,
        priority,
    )?;
    events.append(
        None,
        None,
        "autonomy.queue.enqueued",
        format!(
            "operator enqueued autonomous work item {}: {}",
            short(&item.id),
            truncate(&goal, 100)
        ),
        serde_json::json!({
            "queue_id": item.id,
            "goal": item.goal,
            "kind": item.kind,
            "profile": item.profile,
            "cycles": item.cycles,
            "priority": item.priority,
            "source": "tui",
            "next_command": TUI_QUEUE_STEP_COMMAND,
        }),
    )?;
    Ok(vec![
        styled("Queued autonomous Professor X work", GREEN),
        styled(tui_queue_item_line(&item), CYAN),
        styled(format!("   execute {TUI_QUEUE_STEP_COMMAND}"), DIM),
    ])
}

fn tui_queue_item_line(item: &AutonomyQueueItem) -> String {
    format!(
        "{} queue={} priority={} {}:{} cycles={} {}",
        item.status,
        short(&item.id),
        item.priority,
        item.kind,
        item.profile,
        item.cycles,
        truncate(&item.goal, 72),
    )
}

fn sanitize_tui_goal(goal: &str) -> String {
    goal.chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(300)
        .collect()
}

fn is_tui_step_command(input: &str) -> bool {
    matches!(input.trim(), "/step" | "/step-live")
}

fn run_tui_queue_step_command(events: Arc<EventStore>) {
    let command = TUI_QUEUE_STEP_COMMAND;
    let crate_dir = find_professor_x_crate_dir().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let _ = events.append(
        None,
        None,
        "tui.command.started",
        "running one supervised autonomous queue step from the TUI",
        serde_json::json!({
            "command": command,
            "cwd": crate_dir.display().to_string(),
        }),
    );

    let output = Command::new("cargo")
        .args(["run", "--", "--prof-x-step-live", "1"])
        .current_dir(&crate_dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let _ = events.append(
                None,
                None,
                "tui.command.completed",
                "supervised queue step completed",
                serde_json::json!({
                    "command": command,
                    "status": output.status.code(),
                    "stdout_tail": tail_text(&String::from_utf8_lossy(&output.stdout), 1600),
                    "stderr_tail": tail_text(&String::from_utf8_lossy(&output.stderr), 1600),
                }),
            );
        }
        Ok(output) => {
            let _ = events.append(
                None,
                None,
                "tui.command.failed",
                "supervised queue step failed",
                serde_json::json!({
                    "command": command,
                    "status": output.status.code(),
                    "stdout_tail": tail_text(&String::from_utf8_lossy(&output.stdout), 1600),
                    "stderr_tail": tail_text(&String::from_utf8_lossy(&output.stderr), 1600),
                }),
            );
        }
        Err(err) => {
            let _ = events.append(
                None,
                None,
                "tui.command.failed",
                format!("could not start supervised queue step: {err}"),
                serde_json::json!({
                    "command": command,
                    "cwd": crate_dir.display().to_string(),
                    "error": err.to_string(),
                }),
            );
        }
    }
}

fn find_professor_x_crate_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if is_professor_x_crate(&dir) {
            return Some(dir);
        }
        let nested = dir.join("professor-x");
        if is_professor_x_crate(&nested) {
            return Some(nested);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn is_professor_x_crate(path: &Path) -> bool {
    let manifest = path.join("Cargo.toml");
    let Ok(contents) = std::fs::read_to_string(manifest) else {
        return false;
    };
    contents.contains("name = \"professor-x\"")
}

fn tail_text(text: &str, max_chars: usize) -> String {
    let mut chars: Vec<char> = text.chars().rev().take(max_chars).collect();
    chars.reverse();
    chars.into_iter().collect()
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn bar(v: f32, lo: f32, hi: f32, width: usize) -> String {
    let frac = if hi == lo {
        0.0
    } else {
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    };
    let fill = (frac * width as f32) as usize;
    format!(
        "{}{}",
        "█".repeat(fill),
        "·".repeat(width.saturating_sub(fill))
    )
}

fn draw(f: &mut Frame, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(f.area());
    draw_header(f, rows[0], app);

    if app.show_vitals {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(28)])
            .split(rows[1]);
        draw_transcript(f, cols[0], app);
        draw_vitals_panel(f, cols[1], app);
    } else {
        draw_transcript(f, rows[1], app);
    }
    draw_vitals_footer(f, rows[2], app);
    draw_input(f, rows[3], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let area = area.inner(Margin {
        vertical: 0,
        horizontal: 1,
    });
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(14)])
        .split(area);
    let left = Line::from(vec![
        Span::styled("● ", Style::default().fg(ACCENT)),
        Span::styled(
            "Professor X",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}", app.model), Style::default().fg(DIM)),
    ]);
    let status = if app.working.load(Ordering::Relaxed) {
        Span::styled(
            format!("{} working", SPINNER[app.frame % 4]),
            Style::default().fg(YELLOW),
        )
    } else {
        Span::styled("● ready", Style::default().fg(GREEN))
    };
    f.render_widget(Paragraph::new(left), cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(status)).alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_transcript(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1));
    let inner_h = area.height.saturating_sub(2) as usize;
    let total = app.lines.len();
    let end = total.saturating_sub(app.scroll);
    let start = end.saturating_sub(inner_h);
    let items: Vec<ListItem> = app.lines[start..end]
        .iter()
        .cloned()
        .map(ListItem::new)
        .collect();
    f.render_widget(List::new(items).block(block), area);
}

fn vital_line(label: &str, v: f32, lo: f32, hi: f32, col: Color, val: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<8}"), Style::default().fg(DIM)),
        Span::styled(bar(v, lo, hi, 11), Style::default().fg(col)),
        Span::styled(format!(" {val}"), Style::default().fg(FG)),
    ])
}

fn icol(ics: f32) -> Color {
    if ics >= 0.70 { GREEN } else { RED }
}
fn scol(s: f32) -> Color {
    if s > 0.5 {
        RED
    } else if s > 0.3 {
        YELLOW
    } else {
        GREEN
    }
}

fn draw_vitals_panel(f: &mut Frame, area: Rect, app: &App) {
    let v = &app.vitals;
    let rows = vec![
        vital_line("φ integ", v.phi, 0.0, 3.0, ACCENT, format!("{:.2}", v.phi)),
        vital_line("ICS", v.ics, 0.0, 1.0, icol(v.ics), format!("{:.2}", v.ics)),
        vital_line(
            "valence",
            v.valence,
            -1.0,
            1.0,
            if v.valence >= 0.0 { GREEN } else { RED },
            format!("{:+.2}", v.valence),
        ),
        vital_line(
            "arousal",
            v.arousal,
            0.0,
            1.0,
            YELLOW,
            format!("{:.2}", v.arousal),
        ),
        vital_line(
            "body",
            v.stress,
            0.0,
            1.0,
            scol(v.stress),
            format!("{:.2}", v.stress),
        ),
        Line::from(""),
        styled(format!("phi rounds  {}", v.phi_rounds), DIM),
        styled(format!("episodic    {}", v.episodic), DIM),
        Line::from(""),
        styled(
            format!("queue      {} pending", app.queue.pending),
            DIM,
        ),
        styled(
            format!("latest     {} {}", app.queue.latest_status, app.queue.latest_id),
            CYAN,
        ),
        styled(
            format!("goal       {}", truncate(&app.queue.latest_goal, 32)),
            FG,
        ),
        styled(
            format!("next       {}", app.queue.latest_command),
            DIM,
        ),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            " vitals ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(rows).block(block), area);
}

fn draw_vitals_footer(f: &mut Frame, area: Rect, app: &App) {
    let v = &app.vitals;
    let line = Line::from(vec![
        Span::styled("  φ ", Style::default().fg(DIM)),
        Span::styled(format!("{:.2}", v.phi), Style::default().fg(ACCENT)),
        Span::styled("  ICS ", Style::default().fg(DIM)),
        Span::styled(format!("{:.2}", v.ics), Style::default().fg(icol(v.ics))),
        Span::styled("  body ", Style::default().fg(DIM)),
        Span::styled(
            format!("{:.2}", v.stress),
            Style::default().fg(scol(v.stress)),
        ),
        Span::styled("  queue ", Style::default().fg(DIM)),
        Span::styled(
            format!("{}:{}", app.queue.pending, app.queue.latest_status),
            Style::default().fg(queue_color(&app.queue.latest_status)),
        ),
        Span::styled("        ⇥ Tab for vitals/queue", Style::default().fg(DIM)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn queue_color(status: &str) -> Color {
    match status {
        "passed" | "completed" => GREEN,
        "running" => CYAN,
        "failed" | "rejected" => RED,
        "pending" => YELLOW,
        _ => DIM,
    }
}

fn draw_input(f: &mut Frame, area: Rect, app: &App) {
    let busy = app.working.load(Ordering::Relaxed);
    let content = if busy {
        Line::from(Span::styled(
            "working…  (Esc to quit)",
            Style::default().fg(DIM),
        ))
    } else if app.input.is_empty() {
        Line::from(Span::styled(
            "type a task…  @file pulls a file into context",
            Style::default().fg(DIM),
        ))
    } else {
        Line::from(vec![
            Span::styled(app.input.clone(), Style::default().fg(FG)),
            Span::styled("▏", Style::default().fg(ACCENT)),
        ])
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if busy { DIM } else { ACCENT }))
        .title(Span::styled(
            " ❯ ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(content).block(block), area);
}

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
        tui_loop(
            handle, model, ollama, registry, policy, memory, events, cancel,
        )
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
    app.last_event_id = events
        .tail(1)
        .ok()
        .and_then(|v| v.last().map(|e| e.id))
        .unwrap_or(0);
    refresh_vitals(&memory, &mut app.vitals);
    app.queue = refresh_queue(&memory);

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
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                break;
                            }
                            KeyCode::Tab => app.show_vitals = !app.show_vitals,
                            KeyCode::Enter if !busy && !app.input.trim().is_empty() => {
                                let typed = app.input.trim().to_string();
                                if is_tui_step_command(&typed) {
                                    app.input.clear();
                                    app.scroll = 0;
                                    app.push(Line::from(""));
                                    app.push(Line::from(Span::styled(
                                        format!("▌ {typed}"),
                                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                                    )));
                                    app.push(styled(
                                        "Running one supervised queue step inside this cockpit.",
                                        CYAN,
                                    ));
                                    app.push(styled(
                                        "Watch the transcript for queue, tool, and completion events.",
                                        DIM,
                                    ));
                                    app.working.store(true, Ordering::Relaxed);
                                    let working = Arc::clone(&app.working);
                                    let step_events = Arc::clone(&events);
                                    handle.spawn_blocking(move || {
                                        run_tui_queue_step_command(step_events);
                                        working.store(false, Ordering::Relaxed);
                                    });
                                    continue;
                                }
                                if let Some(lines) = handle_tui_command(&typed, &memory, &events)? {
                                    app.input.clear();
                                    app.scroll = 0;
                                    app.push(Line::from(""));
                                    app.push(Line::from(Span::styled(
                                        format!("▌ {typed}"),
                                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                                    )));
                                    for line in lines {
                                        app.push(line);
                                    }
                                    app.queue = refresh_queue(&memory);
                                    continue;
                                }
                                let task = crate::util::expand_file_refs(&typed);
                                app.input.clear();
                                app.scroll = 0;
                                app.push(Line::from(""));
                                app.push(Line::from(Span::styled(
                                    format!("▌ {typed}"),
                                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                                )));
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
                            KeyCode::PageUp => app.scroll = (app.scroll + 5).min(app.lines.len()),
                            KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(5),
                            _ => {}
                        }
                    }
                }
            }

            if let Ok(evs) = events.tail(80) {
                for e in evs {
                    if e.id > app.last_event_id {
                        app.last_event_id = e.id;
                        if let Some(line) = event_to_line(&e.event_type, &e.summary, &e.payload) {
                            app.push(line);
                        }
                    }
                }
            }

            app.frame += 1;
            if app.frame % 8 == 0 {
                refresh_vitals(&memory, &mut app.vitals);
                app.queue = refresh_queue(&memory);
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue_item(status: &str) -> AutonomyQueueItem {
        let now = chrono::Utc::now();
        AutonomyQueueItem {
            id: "12345678-aaaa-bbbb-cccc-123456789abc".to_string(),
            goal: "make Prof X feel alive in the terminal".to_string(),
            kind: "operator_run".to_string(),
            profile: "commit".to_string(),
            cycles: 3,
            priority: 90,
            status: status.to_string(),
            result_run_id: Some("87654321-bbbb-cccc-dddd-123456789abc".to_string()),
            result_report_path: Some("artifacts/work-loop/2026-06-08/loop.json".to_string()),
            failure_reason: None,
            queued_at: now,
            started_at: Some(now),
            completed_at: if matches!(status, "passed" | "failed" | "completed") {
                Some(now)
            } else {
                None
            },
            updated_at: now,
        }
    }

    #[test]
    fn queue_next_command_steps_pending_work_live() {
        assert_eq!(
            queue_next_command(&queue_item("pending")),
            "cargo run -- --prof-x-step-live 1"
        );
    }

    #[test]
    fn queue_next_command_publishes_passed_work() {
        assert_eq!(
            queue_next_command(&queue_item("passed")),
            "cargo run -- --prof-x-queue-publish 12345678"
        );
    }

    #[test]
    fn queue_event_summary_reads_queue_identity() {
        let line = queue_event_summary(
            "autonomy.queue.completed",
            "completed autonomous queue item",
            &serde_json::json!({"queue_id": "12345678-aaaa-bbbb-cccc-123456789abc"}),
        );

        assert!(line.contains("completed queued work 12345678"));
        assert!(line.contains("completed autonomous queue item"));
    }

    #[test]
    fn queue_signal_preserves_latest_goal_and_command() {
        let item = queue_item("failed");
        let signal = queue_signal_from_item(&item, 2);

        assert_eq!(signal.pending, 2);
        assert_eq!(signal.latest_id, "12345678");
        assert_eq!(signal.latest_status, "failed");
        assert!(signal.latest_goal.contains("feel alive"));
        assert_eq!(
            signal.latest_command,
            "cargo run -- --prof-x-queue-review 12345678"
        );
    }

    #[test]
    fn tui_queue_item_line_is_operator_readable() {
        let line = tui_queue_item_line(&queue_item("pending"));

        assert!(line.contains("pending queue=12345678"));
        assert!(line.contains("operator_run:commit"));
        assert!(line.contains("make Prof X feel alive"));
    }

    #[test]
    fn sanitize_tui_goal_strips_controls_and_bounds_length() {
        let raw = format!("  make\x00progress {}\n", "x".repeat(400));
        let goal = sanitize_tui_goal(&raw);

        assert!(!goal.contains('\0'));
        assert!(!goal.contains('\n'));
        assert!(goal.starts_with("makeprogress"));
        assert!(goal.chars().count() <= 300);
    }

    #[test]
    fn step_command_variants_are_recognized() {
        assert!(is_tui_step_command("/step"));
        assert!(is_tui_step_command(" /step-live "));
        assert!(!is_tui_step_command("/step-live 2"));
        assert!(!is_tui_step_command("/queue"));
    }

    #[test]
    fn tail_text_keeps_suffix_without_splitting_unicode() {
        assert_eq!(tail_text("abcdef", 3), "def");
        assert_eq!(tail_text("αβγδε", 3), "γδε");
    }
}
