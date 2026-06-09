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
use crate::memd::autonomy_queue::{
    autonomy_queue_brief, autonomy_queue_next_command, short_queue_id, AutonomyQueueItem,
    AutonomyQueueStore,
};
use crate::memd::MemoryManager;
use crate::memd::events::EventStore;
use crate::ollama::OllamaClient;
use crate::policyd::PolicyEngine;
use crate::toolbridge::ToolRegistry;

const TICK: Duration = Duration::from_millis(100);
const MAX_LINES: usize = 1200;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠸", "⠴"];
const TUI_QUEUE_STEP_COMMAND: &str = "cargo run -- --prof-x-step-live 1";
const TUI_QUEUE_REVIEW_COMMAND: &str = "cargo run -- --prof-x-queue-review";
const TUI_QUEUE_REPLAY_COMMAND: &str = "cargo run -- --prof-x-queue-replay";
const TUI_QUEUE_PUBLISH_COMMAND: &str = "cargo run -- --prof-x-queue-publish";
const TUI_BRIEF_COMMAND: &str = "cargo run -- --brief";
const TUI_COCKPIT_COMMAND: &str = "cargo run -- --cockpit";
const TUI_WORK_COMMAND: &str = "cargo run -- --work-log";
const TUI_SESSIONS_COMMAND: &str = "cargo run -- --coding-sessions";
const TUI_RUNS_COMMAND: &str = "cargo run -- --work-loops";
const TUI_REVIEW_COMMAND: &str = "cargo run -- --run-review";
const TUI_REPLAY_COMMAND: &str = "cargo run -- --replay";
const TUI_PUBLISH_COMMAND: &str = "cargo run -- --publish-run";
const TUI_TASK_REVIEW_COMMAND: &str = "cargo run -- --task-review";
const TUI_TASK_EVIDENCE_COMMAND: &str = "cargo run -- --task-evidence";
const TUI_SESSION_REVIEW_COMMAND: &str = "cargo run -- --session-review";
const TUI_SESSION_PUBLISH_COMMAND: &str = "cargo run -- --session-publish";
const TUI_PLAN_COMMAND: &str = "cargo run -- --prof-x-plan";
const TUI_PREVIEW_COMMAND: &str = "cargo run -- --prof-x-preview-step";
const TUI_RUN_COMMAND: &str = "cargo run -- --prof-x-run";
const TUI_RUN_COMMIT_COMMAND: &str = "cargo run -- --prof-x-run-commit";

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
        "tui.command.completed" => Some(styled(
            format_tui_command_event(summary, payload),
            GREEN,
        )),
        "tui.command.failed" => Some(styled(format_tui_command_event(summary, payload), RED)),
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
        latest_id: short_queue_id(&item.id),
        latest_goal: item.goal.clone(),
        latest_command: queue_next_command(item),
    }
}

fn queue_next_command(item: &AutonomyQueueItem) -> String {
    autonomy_queue_next_command(item)
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
        styled("   /queue-review [id]    review queue evidence", CYAN),
        styled("   /queue-replay [id]    replay queue timeline", CYAN),
        styled("   /queue-publish [id]   publish linked run evidence", CYAN),
        styled("   /brief /cockpit       show current work state", CYAN),
        styled("   /work [n] /runs [n]   inspect events and run ledger", CYAN),
        styled("   /review [id] /replay [id] /publish [id]", CYAN),
        styled("   /sessions [n] /session-review [id] /session-publish [id]", CYAN),
        styled("   /task-review [id] /task-evidence [id] /inspect [id]", CYAN),
        styled("   /plan /preview        plan or preview autonomous gates", CYAN),
        styled("   /run [n] /run-commit [n] start bounded Prof X runs", CYAN),
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
        lines.push(styled(format!("      next {}", autonomy_queue_brief(&item, 96).next_command), DIM));
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
    let brief = autonomy_queue_brief(item, 72);
    format!(
        "{} queue={} {}",
        item.status,
        brief.queue_id,
        brief.summary,
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
    let input = input.trim();
    strip_tui_command(input, "/step").is_some() || strip_tui_command(input, "/step-live").is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TuiCliCommand {
    label: &'static str,
    command_display: String,
    args: Vec<String>,
}

fn tui_cli_command(input: &str) -> Option<TuiCliCommand> {
    let input = input.trim();
    if input == "/brief" {
        return Some(no_arg_tui_command("brief", "--brief", TUI_BRIEF_COMMAND));
    }
    if input == "/cockpit" {
        return Some(no_arg_tui_command("cockpit snapshot", "--cockpit", TUI_COCKPIT_COMMAND));
    }
    if input == "/plan" {
        return Some(no_arg_tui_command("queue planner", "--prof-x-plan", TUI_PLAN_COMMAND));
    }
    if input == "/preview" {
        return Some(no_arg_tui_command(
            "queue preview",
            "--prof-x-preview-step",
            TUI_PREVIEW_COMMAND,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/step-live") {
        let count = count_arg(rest, 1, 10);
        return Some(TuiCliCommand {
            label: "queue step live",
            command_display: format!("{TUI_QUEUE_STEP_COMMAND} {count}"),
            args: vec!["--prof-x-step-live".to_string(), count],
        });
    }
    if let Some(rest) = strip_tui_command(input, "/step") {
        let count = count_arg(rest, 1, 10);
        return Some(TuiCliCommand {
            label: "queue step",
            command_display: format!("cargo run -- --prof-x-step {count}"),
            args: vec!["--prof-x-step".to_string(), count],
        });
    }
    if let Some(rest) = strip_tui_command(input, "/run-commit") {
        let cycles = count_arg(rest, 5, 50);
        return Some(TuiCliCommand {
            label: "commit-capable Prof X run",
            command_display: format!("{TUI_RUN_COMMIT_COMMAND} {cycles}"),
            args: vec!["--prof-x-run-commit".to_string(), cycles],
        });
    }
    if let Some(rest) = strip_tui_command(input, "/run") {
        let cycles = count_arg(rest, 4, 50);
        return Some(TuiCliCommand {
            label: "core Prof X run",
            command_display: format!("{TUI_RUN_COMMAND} {cycles}"),
            args: vec!["--prof-x-run".to_string(), cycles],
        });
    }
    if let Some(rest) = strip_tui_command(input, "/work") {
        return Some(limit_tui_command("work feed", "--work-log", TUI_WORK_COMMAND, rest, 12));
    }
    if let Some(rest) = strip_tui_command(input, "/sessions") {
        return Some(limit_tui_command(
            "coding sessions",
            "--coding-sessions",
            TUI_SESSIONS_COMMAND,
            rest,
            10,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/runs") {
        return Some(limit_tui_command("run ledger", "--work-loops", TUI_RUNS_COMMAND, rest, 10));
    }
    if let Some(rest) = strip_tui_command(input, "/queue-review") {
        return Some(ref_tui_command(
            "queue review",
            "--prof-x-queue-review",
            TUI_QUEUE_REVIEW_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/queue-replay") {
        return Some(ref_tui_command(
            "queue replay",
            "--prof-x-queue-replay",
            TUI_QUEUE_REPLAY_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/queue-publish") {
        return Some(ref_tui_command(
            "queue publish",
            "--prof-x-queue-publish",
            TUI_QUEUE_PUBLISH_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/session-review") {
        return Some(ref_tui_command(
            "session review",
            "--session-review",
            TUI_SESSION_REVIEW_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/session-publish") {
        return Some(ref_tui_command(
            "session publish",
            "--session-publish",
            TUI_SESSION_PUBLISH_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/task-evidence") {
        return Some(ref_tui_command(
            "task evidence",
            "--task-evidence",
            TUI_TASK_EVIDENCE_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/inspect") {
        return Some(ref_tui_command(
            "task evidence",
            "--inspect",
            "cargo run -- --inspect",
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/task-review") {
        return Some(ref_tui_command(
            "task review",
            "--task-review",
            TUI_TASK_REVIEW_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/review") {
        return Some(ref_tui_command(
            "run review",
            "--run-review",
            TUI_REVIEW_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/replay") {
        return Some(ref_tui_command(
            "run replay",
            "--replay",
            TUI_REPLAY_COMMAND,
            rest,
        ));
    }
    if let Some(rest) = strip_tui_command(input, "/publish") {
        return Some(ref_tui_command(
            "run publish",
            "--publish-run",
            TUI_PUBLISH_COMMAND,
            rest,
        ));
    }
    None
}

fn strip_tui_command<'a>(input: &'a str, command: &str) -> Option<&'a str> {
    if input == command {
        Some("")
    } else {
        input
            .strip_prefix(command)
            .and_then(|rest| rest.strip_prefix(' '))
    }
}

fn no_arg_tui_command(
    label: &'static str,
    flag: impl Into<String>,
    display: impl Into<String>,
) -> TuiCliCommand {
    TuiCliCommand {
        label,
        command_display: display.into(),
        args: vec![flag.into()],
    }
}

fn limit_tui_command(
    label: &'static str,
    flag: &str,
    display: &str,
    rest: &str,
    default_limit: usize,
) -> TuiCliCommand {
    let limit = rest
        .trim()
        .parse::<usize>()
        .unwrap_or(default_limit)
        .clamp(1, 200)
        .to_string();
    TuiCliCommand {
        label,
        command_display: format!("{display} {limit}"),
        args: vec![flag.to_string(), limit],
    }
}

fn count_arg(rest: &str, default_value: usize, max_value: usize) -> String {
    rest.trim()
        .parse::<usize>()
        .unwrap_or(default_value)
        .clamp(1, max_value)
        .to_string()
}

fn ref_tui_command(
    label: &'static str,
    flag: &str,
    display: &str,
    rest: &str,
) -> TuiCliCommand {
    let item_ref = nonempty_or_latest(rest);
    TuiCliCommand {
        label,
        command_display: format!("{display} {item_ref}"),
        args: vec![flag.to_string(), item_ref],
    }
}

fn nonempty_or_latest(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        "latest".to_string()
    } else {
        value.chars().filter(|ch| !ch.is_control()).take(120).collect()
    }
}

fn run_tui_cargo_command(
    events: Arc<EventStore>,
    label: &'static str,
    command_display: String,
    args: Vec<String>,
) {
    let crate_dir = find_professor_x_crate_dir().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let _ = events.append(
        None,
        None,
        "tui.command.started",
        format!("running {label} from the TUI"),
        serde_json::json!({
            "command": command_display,
            "args": args.clone(),
            "cwd": crate_dir.display().to_string(),
        }),
    );

    let output = Command::new("cargo")
        .args(["run", "--"])
        .args(&args)
        .current_dir(&crate_dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = events.append(
                None,
                None,
                "tui.command.completed",
                format!("{label} completed: {}", summarize_command_output(&stdout, &stderr)),
                serde_json::json!({
                    "command": command_display,
                    "status": output.status.code(),
                    "stdout_tail": tail_text(&stdout, 1600),
                    "stderr_tail": tail_text(&stderr, 1600),
                }),
            );
        }
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = events.append(
                None,
                None,
                "tui.command.failed",
                format!("{label} failed: {}", summarize_command_output(&stdout, &stderr)),
                serde_json::json!({
                    "command": command_display,
                    "status": output.status.code(),
                    "stdout_tail": tail_text(&stdout, 1600),
                    "stderr_tail": tail_text(&stderr, 1600),
                }),
            );
        }
        Err(err) => {
            let _ = events.append(
                None,
                None,
                "tui.command.failed",
                format!("could not start {label}: {err}"),
                serde_json::json!({
                    "command": command_display,
                    "cwd": crate_dir.display().to_string(),
                    "error": err.to_string(),
                }),
            );
        }
    }
}

fn summarize_command_output(stdout: &str, stderr: &str) -> String {
    first_meaningful_line(stdout)
        .or_else(|| first_meaningful_line(stderr))
        .unwrap_or_else(|| "no output".to_string())
}

fn first_meaningful_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| truncate(line, 140))
}

fn format_tui_command_event(summary: &str, payload: &Value) -> String {
    let command = payload
        .get("command")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if command.is_empty() {
        format!("  {summary}")
    } else {
        format!("  {summary} [{command}]")
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
                                    let command = tui_cli_command(&typed)
                                        .expect("step commands are mapped to CLI commands");
                                    app.input.clear();
                                    app.scroll = 0;
                                    app.push(Line::from(""));
                                    app.push(Line::from(Span::styled(
                                        format!("▌ {typed}"),
                                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                                    )));
                                    app.push(styled(
                                        format!(
                                            "Running {} inside this cockpit.",
                                            command.label
                                        ),
                                        CYAN,
                                    ));
                                    app.push(styled(
                                        format!("   {}", command.command_display),
                                        DIM,
                                    ));
                                    app.push(styled(
                                        "Watch the transcript for queue, tool, and completion events.",
                                        DIM,
                                    ));
                                    app.working.store(true, Ordering::Relaxed);
                                    let working = Arc::clone(&app.working);
                                    let step_events = Arc::clone(&events);
                                    handle.spawn_blocking(move || {
                                        run_tui_cargo_command(
                                            step_events,
                                            command.label,
                                            command.command_display,
                                            command.args,
                                        );
                                        working.store(false, Ordering::Relaxed);
                                    });
                                    continue;
                                }
                                if let Some(command) = tui_cli_command(&typed) {
                                    app.input.clear();
                                    app.scroll = 0;
                                    app.push(Line::from(""));
                                    app.push(Line::from(Span::styled(
                                        format!("▌ {typed}"),
                                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                                    )));
                                    app.push(styled(
                                        format!(
                                            "Running {} inside this cockpit.",
                                            command.label
                                        ),
                                        CYAN,
                                    ));
                                    app.push(styled(
                                        format!("   {}", command.command_display),
                                        DIM,
                                    ));
                                    app.working.store(true, Ordering::Relaxed);
                                    let working = Arc::clone(&app.working);
                                    let command_events = Arc::clone(&events);
                                    handle.spawn_blocking(move || {
                                        run_tui_cargo_command(
                                            command_events,
                                            command.label,
                                            command.command_display,
                                            command.args,
                                        );
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
    fn queue_next_command_reviews_passed_work_before_publish() {
        assert_eq!(
            queue_next_command(&queue_item("passed")),
            "cargo run -- --prof-x-queue-review 12345678"
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
        assert!(is_tui_step_command("/step-live 2"));
        assert!(!is_tui_step_command("/queue"));
    }

    #[test]
    fn tail_text_keeps_suffix_without_splitting_unicode() {
        assert_eq!(tail_text("abcdef", 3), "def");
        assert_eq!(tail_text("αβγδε", 3), "γδε");
    }

    #[test]
    fn tui_cli_command_builds_queue_lifecycle_args() {
        let review = tui_cli_command("/queue-review abc123").expect("review command");
        assert_eq!(review.label, "queue review");
        assert_eq!(review.args, vec!["--prof-x-queue-review", "abc123"]);
        assert_eq!(
            review.command_display,
            "cargo run -- --prof-x-queue-review abc123"
        );

        let publish = tui_cli_command("/queue-publish").expect("publish command");
        assert_eq!(publish.args, vec!["--prof-x-queue-publish", "latest"]);
    }

    #[test]
    fn tui_cli_command_sanitizes_queue_ref() {
        let command = tui_cli_command("/queue-replay abc\x00def\n").expect("replay command");
        assert_eq!(command.args, vec!["--prof-x-queue-replay", "abcdef"]);
    }

    #[test]
    fn tui_cli_command_builds_operator_inspection_args() {
        let brief = tui_cli_command("/brief").expect("brief command");
        assert_eq!(brief.args, vec!["--brief"]);

        let work = tui_cli_command("/work 7").expect("work command");
        assert_eq!(work.label, "work feed");
        assert_eq!(work.args, vec!["--work-log", "7"]);

        let runs = tui_cli_command("/runs").expect("runs command");
        assert_eq!(runs.args, vec!["--work-loops", "10"]);

        let review = tui_cli_command("/review abc123").expect("review command");
        assert_eq!(review.args, vec!["--run-review", "abc123"]);

        let evidence = tui_cli_command("/inspect").expect("inspect command");
        assert_eq!(evidence.args, vec!["--inspect", "latest"]);
    }

    #[test]
    fn tui_cli_command_builds_step_and_run_args() {
        let step = tui_cli_command("/step 3").expect("step command");
        assert_eq!(step.label, "queue step");
        assert_eq!(step.args, vec!["--prof-x-step", "3"]);

        let live_step = tui_cli_command("/step-live 99").expect("live step command");
        assert_eq!(live_step.args, vec!["--prof-x-step-live", "10"]);

        let run = tui_cli_command("/run").expect("run command");
        assert_eq!(run.args, vec!["--prof-x-run", "4"]);

        let commit_run = tui_cli_command("/run-commit 9").expect("commit run command");
        assert_eq!(commit_run.label, "commit-capable Prof X run");
        assert_eq!(commit_run.args, vec!["--prof-x-run-commit", "9"]);
    }

    #[test]
    fn tui_cli_command_builds_session_and_planner_args() {
        let sessions = tui_cli_command("/sessions 3").expect("sessions command");
        assert_eq!(sessions.args, vec!["--coding-sessions", "3"]);

        let session_review =
            tui_cli_command("/session-review cafebabe").expect("session review command");
        assert_eq!(session_review.args, vec!["--session-review", "cafebabe"]);

        let plan = tui_cli_command("/plan").expect("plan command");
        assert_eq!(plan.args, vec!["--prof-x-plan"]);

        let preview = tui_cli_command("/preview").expect("preview command");
        assert_eq!(preview.args, vec!["--prof-x-preview-step"]);
    }

    #[test]
    fn tui_cli_command_does_not_match_partial_prefixes() {
        assert!(tui_cli_command("/workflow").is_none());
        assert!(tui_cli_command("/reviewer latest").is_none());
        assert!(tui_cli_command("/session-reviewer latest").is_none());
    }

    #[test]
    fn summarize_command_output_prefers_stdout_then_stderr() {
        assert_eq!(summarize_command_output("\nready\n", "error"), "ready");
        assert_eq!(summarize_command_output("", "\nfailed\n"), "failed");
        assert_eq!(summarize_command_output("", ""), "no output");
    }
}
