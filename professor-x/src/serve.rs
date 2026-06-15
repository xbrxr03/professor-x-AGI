//! `profx --serve` — a local web UI for the harness.
//!
//! A small axum server that exposes Professor X's existing event stream + vitals
//! over HTTP and runs tasks, plus a self-contained vanilla-JS frontend so it
//! works out of the box. The API is the contract: a generative-UI frontend
//! (e.g. wandb/OpenUI) can replace the built-in page by pointing at the same
//! endpoints.
//!
//!   GET  /                 → the built-in dashboard page
//!   GET  /api/events?since=N → events with id > N  (live activity)
//!   GET  /api/vitals       → φ, ICS, affect, body, counts
//!   GET  /api/status       → current run, latest task, gates, queue, coding session, work events
//!   POST /api/task {task}  → run a task (spawned; watch via /api/events)

use anyhow::Result;
use axum::{
    extract::{Query, State},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::agentd::graph::{TaskNode, TaskType};
use crate::agentd::react::ReactLoop;
use crate::memd::autonomy_queue::{AutonomyQueueItem, AutonomyQueueStore};
use crate::memd::coding_sessions::{CodingSessionRecord, CodingSessionStore};
use crate::memd::coding_smoke::{CodingSmokeRecord, CodingSmokeStore};
use crate::memd::events::EventStore;
use crate::memd::task_runs::{TaskRun, TaskRunStore};
use crate::memd::work_loops::{
    WorkLoopGateRecord, WorkLoopGateStore, WorkLoopRunRecord, WorkLoopRunStore,
};
use crate::memd::MemoryManager;
use crate::ollama::OllamaClient;
use crate::policyd::PolicyEngine;
use crate::toolbridge::ToolRegistry;

#[derive(Clone)]
struct AppState {
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
    working: Arc<AtomicBool>,
    model: String,
}

pub async fn run_serve(
    ollama: Arc<OllamaClient>,
    registry: Arc<std::sync::RwLock<ToolRegistry>>,
    policy: Arc<PolicyEngine>,
    memory: Arc<MemoryManager>,
    events: Arc<EventStore>,
    cancel: CancellationToken,
) -> Result<()> {
    let model = ollama.model_name().to_string();
    let state = AppState {
        ollama,
        registry,
        policy,
        memory,
        events,
        cancel,
        working: Arc::new(AtomicBool::new(false)),
        model,
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/events", get(api_events))
        .route("/api/vitals", get(api_vitals))
        .route("/api/status", get(api_status))
        .route("/api/task", post(api_task))
        .with_state(state);

    let addr = "127.0.0.1:8787";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("\n  Professor X web UI →  http://{addr}\n  (Ctrl-C to stop)\n");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_events(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let since: i64 = q.get("since").and_then(|s| s.parse().ok()).unwrap_or(0);
    let evs = st.events.tail(120).unwrap_or_default();
    let out: Vec<serde_json::Value> = evs
        .into_iter()
        .filter(|e| e.id > since)
        .map(|e| json!({"id": e.id, "type": e.event_type, "summary": e.summary}))
        .collect();
    Json(json!({"events": out, "working": st.working.load(Ordering::Relaxed)}))
}

async fn api_vitals(State(st): State<AppState>) -> Json<serde_json::Value> {
    let db = match st.memory.db.lock() {
        Ok(d) => d,
        Err(_) => return Json(json!({})),
    };
    let f = |sql: &str| -> f64 { db.query_row(sql, [], |r| r.get::<_, f64>(0)).unwrap_or(0.0) };
    let i = |sql: &str| -> i64 { db.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    let (mut stress, mut lat) = (0.0f64, 0.0f64);
    if let Ok((l, tok, mem, health)) = db.query_row(
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
        lat = (l / 10000.0).min(1.0);
        stress = 0.35 * lat + 0.25 * tok + 0.20 * mem + 0.20 * (1.0 - health);
    }
    Json(json!({
        "model": st.model,
        "working": st.working.load(Ordering::Relaxed),
        "phi": f("SELECT phi FROM phi_rounds ORDER BY round DESC LIMIT 1"),
        "ics": f("SELECT score FROM ics_scores ORDER BY id DESC LIMIT 1"),
        "valence": f("SELECT valence FROM affect_states ORDER BY id DESC LIMIT 1"),
        "arousal": f("SELECT arousal FROM affect_states ORDER BY id DESC LIMIT 1"),
        "stress": stress,
        "episodic": i("SELECT COUNT(*) FROM episodic"),
        "phi_rounds": i("SELECT COUNT(*) FROM phi_rounds"),
    }))
}

async fn api_status(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(status_payload(&st).unwrap_or_else(|err| {
        json!({
            "schema": "professor_x.web_status.v1",
            "ok": false,
            "error": err.to_string(),
        })
    }))
}

fn status_payload(st: &AppState) -> Result<serde_json::Value> {
    let latest_task_run = TaskRunStore::new(Arc::clone(&st.memory.db)).latest()?;
    let latest_run = WorkLoopRunStore::new(Arc::clone(&st.memory.db)).latest()?;
    let gate_store = WorkLoopGateStore::new(Arc::clone(&st.memory.db));
    let latest_gate = gate_store.latest()?;
    let gates = latest_run
        .as_ref()
        .map(|run| gate_store.recent_for_run(&run.run_id, 8))
        .transpose()?
        .unwrap_or_default();
    let queue = AutonomyQueueStore::new(Arc::clone(&st.memory.db)).recent(5)?;
    let latest_session = CodingSessionStore::new(Arc::clone(&st.memory.db)).latest()?;
    let latest_smoke = CodingSmokeStore::new(Arc::clone(&st.memory.db)).latest()?;
    let work_events = st.events.work_tail(24)?;

    Ok(json!({
        "schema": "professor_x.web_status.v1",
        "ok": true,
        "model": st.model,
        "working": st.working.load(Ordering::Relaxed),
        "state": work_state(latest_run.as_ref(), latest_gate.as_ref(), st.working.load(Ordering::Relaxed)),
        "now": work_now(&work_events, latest_gate.as_ref(), latest_session.as_ref()),
        "latest_task_run": latest_task_run.as_ref().map(task_run_json),
        "current_run": latest_run.as_ref().map(run_json),
        "active_gate": latest_gate.as_ref().map(gate_json),
        "gate_ledger": gates.iter().map(gate_json).collect::<Vec<_>>(),
        "queue": queue.iter().map(queue_json).collect::<Vec<_>>(),
        "latest_coding_session": latest_session.as_ref().map(coding_session_json),
        "latest_coding_smoke": latest_smoke.as_ref().map(coding_smoke_json),
        "work_events": work_events.iter().map(|event| json!({
            "id": event.id,
            "timestamp": event.timestamp.to_rfc3339(),
            "type": event.event_type,
            "summary": event.summary,
        })).collect::<Vec<_>>(),
        "commands": [
            "cargo run -- --tui",
            "cargo run -- --cockpit",
            "cargo run -- --status-json",
            "cargo run -- --prof-x-step-live 1",
            "cargo run -- --prof-x-live-publish 6"
        ],
    }))
}

fn work_state(
    latest_run: Option<&WorkLoopRunRecord>,
    latest_gate: Option<&WorkLoopGateRecord>,
    web_task_running: bool,
) -> &'static str {
    if web_task_running || latest_gate.is_some_and(|gate| gate.status == "running") {
        "RUNNING"
    } else if latest_run.is_some_and(|run| run.failed_cycles > 0) {
        "NEEDS-REVIEW"
    } else if latest_run.is_some() {
        "READY"
    } else {
        "IDLE"
    }
}

fn work_now(
    events: &[crate::memd::events::AgentEvent],
    latest_gate: Option<&WorkLoopGateRecord>,
    latest_session: Option<&CodingSessionRecord>,
) -> String {
    if let Some(gate) = latest_gate.filter(|gate| gate.status == "running") {
        return format!("{}: {}", gate.kind, gate.label);
    }
    if let Some(event) = events.last() {
        return event.summary.clone();
    }
    if let Some(session) = latest_session {
        return format!("latest coding session: {}", session.goal);
    }
    "waiting for work".to_string()
}

fn run_json(run: &WorkLoopRunRecord) -> serde_json::Value {
    json!({
        "run_id": run.run_id,
        "short_id": short_id(&run.run_id),
        "kind": run.run_kind,
        "profile": run.profile,
        "progress": format!("{}/{}", run.completed_cycles, run.requested_cycles),
        "passed_cycles": run.passed_cycles,
        "failed_cycles": run.failed_cycles,
        "report_path": run.report_path,
        "started_at": run.started_at.to_rfc3339(),
        "completed_at": run.completed_at.to_rfc3339(),
    })
}

fn task_run_json(run: &TaskRun) -> serde_json::Value {
    json!({
        "task_id": run.task_id,
        "short_id": short_id(&run.task_id),
        "status": run.status,
        "task_type": run.task_type,
        "priority": run.priority,
        "attempt_count": run.attempt_count,
        "step_count": run.step_count,
        "score": run.outcome_score,
        "failure_class": run.failure_class.map(|class| class.as_str().to_string()),
        "failure_mode": run.failure_mode,
        "last_tool": run.last_tool,
        "last_summary": run.last_summary,
        "transcript_path": run.transcript_path,
        "updated_at": run.updated_at.to_rfc3339(),
        "completed_at": run.completed_at.map(|ts| ts.to_rfc3339()),
    })
}

fn gate_json(gate: &WorkLoopGateRecord) -> serde_json::Value {
    json!({
        "run_id": gate.run_id,
        "short_run_id": short_id(&gate.run_id),
        "cycle": gate.cycle,
        "kind": gate.kind,
        "label": gate.label,
        "status": gate.status,
        "passed": gate.passed,
        "report_path": gate.report_path,
        "transcript_path": gate.transcript_path,
        "detail": gate.detail,
        "updated_at": gate.updated_at.to_rfc3339(),
    })
}

fn queue_json(item: &AutonomyQueueItem) -> serde_json::Value {
    json!({
        "id": item.id,
        "short_id": short_id(&item.id),
        "goal": item.goal,
        "kind": item.kind,
        "profile": item.profile,
        "cycles": item.cycles,
        "priority": item.priority,
        "status": item.status,
        "result_run_id": item.result_run_id,
        "result_report_path": item.result_report_path,
        "failure_reason": item.failure_reason,
        "updated_at": item.updated_at.to_rfc3339(),
    })
}

fn coding_session_json(session: &CodingSessionRecord) -> serde_json::Value {
    json!({
        "id": session.id,
        "short_id": short_id(&session.id),
        "goal": session.goal,
        "status": session.status,
        "workspace": session.workspace,
        "checks": session.checks,
        "artifacts": session.artifacts,
        "session_report_path": session.session_report_path,
        "smoke_report_path": session.smoke_report_path,
        "transcript_path": session.transcript_path,
        "failure_reason": session.failure_reason,
        "generated_at": session.generated_at.to_rfc3339(),
    })
}

fn coding_smoke_json(smoke: &CodingSmokeRecord) -> serde_json::Value {
    json!({
        "id": smoke.id,
        "passed": smoke.passed,
        "initial_test_failed": smoke.initial_test_failed,
        "edit_applied": smoke.edit_applied,
        "final_test_passed": smoke.final_test_passed,
        "report_path": smoke.report_path,
        "transcript_path": smoke.transcript_path,
        "generated_at": smoke.generated_at.to_rfc3339(),
    })
}

fn short_id(value: &str) -> String {
    value.chars().take(8).collect()
}

#[derive(Deserialize)]
struct TaskBody {
    task: String,
}

async fn api_task(
    State(st): State<AppState>,
    Json(body): Json<TaskBody>,
) -> Json<serde_json::Value> {
    let typed = body.task.trim().to_string();
    if typed.is_empty() {
        return Json(json!({"ok": false, "error": "empty task"}));
    }
    let task = crate::util::expand_file_refs(&typed); // @file → inline context
    if st.working.swap(true, Ordering::Relaxed) {
        return Json(json!({"ok": false, "error": "already working"}));
    }
    let (o, r, p, m, e, c, w) = (
        Arc::clone(&st.ollama),
        Arc::clone(&st.registry),
        Arc::clone(&st.policy),
        Arc::clone(&st.memory),
        Arc::clone(&st.events),
        st.cancel.clone(),
        Arc::clone(&st.working),
    );
    tokio::spawn(async move {
        let react = ReactLoop::new(o, r, p, m, c).with_events(e);
        let mut t = TaskNode::new(task, TaskType::UserRequest, 100);
        let _ = react.run(&mut t).await;
        w.store(false, Ordering::Relaxed);
    });
    Json(json!({"ok": true}))
}

const INDEX_HTML: &str = r#"<!doctype html><html><head><meta charset=utf-8>
<title>Professor X</title><meta name=viewport content="width=device-width,initial-scale=1">
<style>
:root{--bg:#0d0d12;--panel:#16161f;--panel2:#101018;--line:#262633;--fg:#e6e6f0;--dim:#8a8aa0;--mag:#c678dd;--cyan:#56b6c2;--grn:#98c379;--red:#e06c75;--yel:#e5c07b}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 ui-monospace,Menlo,Consolas,monospace}
header{padding:10px 16px;border-bottom:1px solid var(--line);display:flex;gap:14px;align-items:center}
.brand{color:var(--mag);font-weight:700}.dim{color:var(--dim)}
.wrap{display:grid;grid-template-columns:1fr 340px;gap:0;height:calc(100vh - 52px - 64px)}
main{overflow:auto;padding:14px 16px}.grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-bottom:12px}
.panel{border:1px solid var(--line);background:var(--panel2);border-radius:8px;padding:12px;min-height:112px}.panel.wide{grid-column:1/-1}
.kv{display:grid;grid-template-columns:92px 1fr;gap:4px 10px}.key{color:var(--dim)}.value{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
#feed{overflow:auto;max-height:42vh;border-top:1px solid var(--line);padding-top:10px}.ev{white-space:pre-wrap;margin:1px 0}
aside{border-left:1px solid var(--line);padding:14px 16px;background:var(--panel)}
.vit{margin:6px 0}.bar{display:inline-block;height:9px;border-radius:3px;vertical-align:middle}
.lbl{display:inline-block;width:62px;color:var(--dim)}
footer{position:fixed;bottom:0;left:0;right:0;padding:12px 16px;border-top:1px solid var(--line);background:var(--bg)}
input{width:100%;background:var(--panel);border:1px solid var(--line);color:var(--fg);padding:10px;border-radius:8px;font:inherit}
.s-grn{color:var(--grn)}.s-red{color:var(--red)}.s-cyan{color:var(--cyan)}.s-mag{color:var(--mag)}.s-yel{color:var(--yel)}.s-dim{color:var(--dim)}
h3{margin:0 0 8px;font-size:12px;color:var(--dim);text-transform:uppercase;letter-spacing:.08em}
button{background:var(--panel2);border:1px solid var(--line);color:var(--fg);padding:6px 8px;border-radius:6px;font:inherit;margin:3px 4px 3px 0}
@media(max-width:900px){.wrap{grid-template-columns:1fr;height:auto}.grid{grid-template-columns:1fr}aside{border-left:0;border-top:1px solid var(--line);padding-bottom:84px}footer{position:sticky}}
</style></head><body>
<header><span class=brand>PROFESSOR X</span><span id=model class=dim></span><span id=status class=dim>ready</span></header>
<div class=wrap>
  <main>
    <div class=grid>
      <section class="panel wide"><h3>live state</h3><div id=state class=kv></div></section>
      <section class=panel><h3>current run</h3><div id=run class=kv></div></section>
      <section class=panel><h3>active gate</h3><div id=gate class=kv></div></section>
      <section class=panel><h3>latest task</h3><div id=taskrun class=kv></div></section>
      <section class=panel><h3>autonomy queue</h3><div id=queue></div></section>
      <section class=panel><h3>coding session</h3><div id=session class=kv></div></section>
      <section class="panel wide"><h3>operator commands</h3><div id=commands></div></section>
    </div>
    <h3>work stream</h3><div id=feed></div>
  </main>
  <aside>
    <h3>consciousness vitals</h3><div id=vitals></div>
    <h3 style=margin-top:18px>stats</h3><div id=stats class=dim></div>
  </aside>
</div>
<footer><input id=task placeholder="Type a task and press Enter…" autofocus></footer>
<script>
let since=0;const feed=document.getElementById('feed');
const cls={'task.succeeded':'s-grn','tool.succeeded':'s-grn','task.failed':'s-red','policy.denied':'s-red','tool.started':'s-cyan','agent.delegate':'s-mag','react.duplicate_action':'s-yel','llm.response':'s-dim'};
function bar(v,lo,hi,col){let f=Math.max(0,Math.min(1,(v-lo)/(hi-lo)));return `<span class=bar style="width:${(f*120)|0}px;background:${col}"></span>`}
function esc(s){return (s??'').toString().replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
function kv(obj,keys){return keys.map(([k,l])=>`<span class=key>${l}</span><span class=value title="${esc(obj?.[k])}">${esc(obj?.[k]??'—')}</span>`).join('')}
async function pollEvents(){try{let r=await fetch('/api/events?since='+since);let d=await r.json();
 for(const e of d.events){since=Math.max(since,e.id);let div=document.createElement('div');div.className='ev '+(cls[e.type]||'s-dim');div.textContent=e.type.padEnd(22)+' '+e.summary;feed.appendChild(div)}
 if(d.events.length)feed.scrollTop=feed.scrollHeight;
 document.getElementById('status').textContent=d.working?'● working':'ready'}catch(e){}}
async function pollStatus(){try{let s=await(await fetch('/api/status')).json();if(!s.ok)return;
 document.getElementById('status').textContent=(s.working?'● ':'')+s.state;
 document.getElementById('state').innerHTML=kv(s,[['state','state'],['now','now'],['model','model']]);
 document.getElementById('run').innerHTML=s.current_run?kv(s.current_run,[['short_id','id'],['profile','profile'],['progress','progress'],['failed_cycles','failed'],['report_path','report']]):'<span class=s-dim>no run recorded</span>';
 document.getElementById('gate').innerHTML=s.active_gate?kv(s.active_gate,[['kind','kind'],['label','label'],['status','status'],['detail','detail'],['report_path','report']]):'<span class=s-dim>no gate recorded</span>';
 document.getElementById('taskrun').innerHTML=s.latest_task_run?kv(s.latest_task_run,[['short_id','id'],['status','status'],['failure_class','class'],['last_tool','tool'],['last_summary','summary']]):'<span class=s-dim>no task recorded</span>';
 document.getElementById('queue').innerHTML=(s.queue||[]).map(q=>`<div class=ev><span class=s-cyan>${esc(q.short_id)}</span> ${esc(q.status)} p${esc(q.priority)} ${esc(q.profile)} · ${esc(q.goal)}</div>`).join('')||'<span class=s-dim>queue empty</span>';
 document.getElementById('session').innerHTML=s.latest_coding_session?kv(s.latest_coding_session,[['short_id','id'],['status','status'],['goal','goal'],['session_report_path','report']]):'<span class=s-dim>no coding session recorded</span>';
 document.getElementById('commands').innerHTML=(s.commands||[]).map(c=>`<button title="${esc(c)}">${esc(c.replace('cargo run -- ',''))}</button>`).join('');
 if(s.work_events?.length){feed.innerHTML='';for(const e of s.work_events){let div=document.createElement('div');div.className='ev '+(cls[e.type]||'s-dim');div.textContent=e.type.padEnd(22)+' '+e.summary;feed.appendChild(div)}feed.scrollTop=feed.scrollHeight}
}catch(e){}}
async function pollVitals(){try{let v=await(await fetch('/api/vitals')).json();
 document.getElementById('model').textContent='· '+(v.model||'');
 const col=v.ics>=.7?'#98c379':'#e06c75';
 document.getElementById('vitals').innerHTML=
  `<div class=vit><span class=lbl>φ integ</span>${bar(v.phi,0,3,'#c678dd')} ${(v.phi||0).toFixed(2)}</div>`+
  `<div class=vit><span class=lbl>ICS</span>${bar(v.ics,0,1,col)} ${(v.ics||0).toFixed(2)}</div>`+
  `<div class=vit><span class=lbl>valence</span>${bar(v.valence,-1,1,v.valence>=0?'#98c379':'#e06c75')} ${(v.valence||0).toFixed(2)}</div>`+
  `<div class=vit><span class=lbl>arousal</span>${bar(v.arousal,0,1,'#e5c07b')} ${(v.arousal||0).toFixed(2)}</div>`+
  `<div class=vit><span class=lbl>body</span>${bar(v.stress,0,1,v.stress>.5?'#e06c75':'#98c379')} ${(v.stress||0).toFixed(2)}</div>`;
 document.getElementById('stats').textContent='episodic '+v.episodic+'  ·  phi rounds '+v.phi_rounds}catch(e){}}
document.getElementById('task').addEventListener('keydown',async e=>{if(e.key==='Enter'){let t=e.target.value.trim();if(!t)return;
 let div=document.createElement('div');div.className='ev';div.style.color='#fff';div.textContent='▶ '+t;feed.appendChild(div);feed.scrollTop=feed.scrollHeight;
 e.target.value='';await fetch('/api/task',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({task:t})})}});
setInterval(pollEvents,600);setInterval(pollStatus,1200);setInterval(pollVitals,1200);pollEvents();pollStatus();pollVitals();
</script></body></html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_dashboard_polls_structured_status() {
        assert!(INDEX_HTML.contains("/api/status"));
        assert!(INDEX_HTML.contains("current run"));
        assert!(INDEX_HTML.contains("active gate"));
        assert!(INDEX_HTML.contains("latest task"));
        assert!(INDEX_HTML.contains("autonomy queue"));
        assert!(INDEX_HTML.contains("coding session"));
    }

    #[test]
    fn web_status_state_prefers_active_work() {
        assert_eq!(work_state(None, None, false), "IDLE");
        assert_eq!(work_state(None, None, true), "RUNNING");
    }

    #[test]
    fn web_status_short_ids_are_operator_sized() {
        assert_eq!(short_id("123456789abcdef"), "12345678");
    }
}
