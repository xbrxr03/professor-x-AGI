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
use crate::memd::events::EventStore;
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
        |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?, r.get::<_, f64>(3)?)),
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

#[derive(Deserialize)]
struct TaskBody {
    task: String,
}

async fn api_task(
    State(st): State<AppState>,
    Json(body): Json<TaskBody>,
) -> Json<serde_json::Value> {
    let task = body.task.trim().to_string();
    if task.is_empty() {
        return Json(json!({"ok": false, "error": "empty task"}));
    }
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
:root{--bg:#0d0d12;--panel:#16161f;--line:#262633;--fg:#e6e6f0;--dim:#8a8aa0;--mag:#c678dd;--cyan:#56b6c2;--grn:#98c379;--red:#e06c75;--yel:#e5c07b}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 ui-monospace,Menlo,Consolas,monospace}
header{padding:10px 16px;border-bottom:1px solid var(--line);display:flex;gap:14px;align-items:center}
.brand{color:var(--mag);font-weight:700}.dim{color:var(--dim)}
.wrap{display:grid;grid-template-columns:1fr 300px;gap:0;height:calc(100vh - 52px - 64px)}
#feed{overflow:auto;padding:10px 16px}.ev{white-space:pre-wrap;margin:1px 0}
aside{border-left:1px solid var(--line);padding:14px 16px;background:var(--panel)}
.vit{margin:6px 0}.bar{display:inline-block;height:9px;border-radius:3px;vertical-align:middle}
.lbl{display:inline-block;width:62px;color:var(--dim)}
footer{position:fixed;bottom:0;left:0;right:0;padding:12px 16px;border-top:1px solid var(--line);background:var(--bg)}
input{width:100%;background:var(--panel);border:1px solid var(--line);color:var(--fg);padding:10px;border-radius:8px;font:inherit}
.s-grn{color:var(--grn)}.s-red{color:var(--red)}.s-cyan{color:var(--cyan)}.s-mag{color:var(--mag)}.s-yel{color:var(--yel)}.s-dim{color:var(--dim)}
h3{margin:0 0 8px;font-size:12px;color:var(--dim);text-transform:uppercase;letter-spacing:.08em}
</style></head><body>
<header><span class=brand>PROFESSOR X</span><span id=model class=dim></span><span id=status class=dim>ready</span></header>
<div class=wrap>
  <div id=feed></div>
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
async function pollEvents(){try{let r=await fetch('/api/events?since='+since);let d=await r.json();
 for(const e of d.events){since=Math.max(since,e.id);let div=document.createElement('div');div.className='ev '+(cls[e.type]||'s-dim');div.textContent=e.type.padEnd(22)+' '+e.summary;feed.appendChild(div)}
 if(d.events.length)feed.scrollTop=feed.scrollHeight;
 document.getElementById('status').textContent=d.working?'● working':'ready'}catch(e){}}
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
setInterval(pollEvents,600);setInterval(pollVitals,1200);pollEvents();pollVitals();
</script></body></html>"#;
