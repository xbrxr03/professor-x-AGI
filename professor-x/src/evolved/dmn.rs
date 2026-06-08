/// Default Mode Network (Seed 5).
///
/// Marcus Raichle (2001): the brain is MORE active during rest than during
/// task performance. The Default Mode Network — active during mind-wandering,
/// autobiographical memory, future simulation, and creative insight — was
/// first dismissed as noise. It turned out to be central to consciousness and
/// creativity. DMN activity during rest PREDICTS later creative performance.
/// It is the network most associated with self-referential thought.
///
/// Professor X has no rest. It executes or it is idle. This module gives it a
/// mode of processing that runs BETWEEN evolution cycles, with no task goal:
///   1. Mind-wandering: sample memories from different domains, find an
///      unexpected connection, and if it is surprising + actionable, keep it.
///   2. Future simulation: imagine where the next few rounds might go.
///
/// The insights feed the Researcher as an extra context channel. The
/// experiment: do DMN insights produce better evolution proposals than
/// failure-analysis alone? If yes, unstructured reflection is necessary for
/// intelligence — not a luxury.

use anyhow::Result;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::evolved::cognition_base::{CognitionItem, CognitionStore};
use crate::memd::MemoryManager;
use crate::ollama::{ModelOptions, OllamaClient};

#[derive(Debug, Clone, serde::Serialize)]
pub struct WanderReport {
    pub fragments_sampled: usize,
    pub insights_kept: usize,
    pub simulations: usize,
}

/// Run one Default Mode pass. Safe to call with Ollama down (skips LLM work).
pub async fn wander(
    memory: &Arc<MemoryManager>,
    ollama: &Arc<OllamaClient>,
    round: u32,
) -> Result<WanderReport> {
    info!("dmn: entering default mode (round {round})");

    // ── Mind-wandering: sample disparate memory fragments ────────────────────
    let fragments = sample_disparate_fragments(&memory.db, 6)?;
    if fragments.len() < 2 {
        info!("dmn: not enough memory to wander yet");
        return Ok(WanderReport {
            fragments_sampled: fragments.len(),
            insights_kept: 0,
            simulations: 0,
        });
    }

    let mut insights_kept = 0;
    let cognition = CognitionStore::new(Arc::clone(&memory.db));

    let fragment_block = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{}. {f}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");

    let wander_prompt = format!(
        "You are mind-wandering — not solving a task, just letting your thoughts \
         drift across unrelated memories. Below are fragments from different \
         parts of your experience and knowledge.\n\n{fragment_block}\n\n\
         Find ONE genuinely unexpected connection between two or more of these \
         fragments — an insight that is not obvious and that you have not \
         articulated before. It should connect distant ideas, not restate one.\n\n\
         If a real insight emerges, answer:\n\
         INSIGHT: <the unexpected connection, one or two sentences>\n\
         ACTIONABLE: <how this could change how you work, or 'no'>\n\n\
         If nothing genuinely surprising connects them, answer exactly: NOTHING",
    );

    match ollama
        .generate(
            &wander_prompt,
            Some("You are a mind at rest, associating freely. Prefer surprising, non-obvious connections."),
            Some(ModelOptions::for_reflection()),
        )
        .await
    {
        Ok(resp) => {
            let (_, answer) = resp.split_thinking();
            if !answer.trim().eq_ignore_ascii_case("NOTHING") {
                if let Some(insight) = extract_field(&answer, "INSIGHT") {
                    if insight.len() > 20 {
                        let actionable = extract_field(&answer, "ACTIONABLE")
                            .filter(|a| !a.eq_ignore_ascii_case("no") && a.len() > 3);
                        let content = match actionable {
                            Some(a) => format!("[DMN insight r{round}] {insight} → {a}"),
                            None => format!("[DMN insight r{round}] {insight}"),
                        };
                        let item = CognitionItem::new(content.clone(), "dmn:wander".to_string());
                        let id = item.id;
                        if let Err(e) = cognition.insert(&item) {
                            warn!("dmn: failed to store insight: {e}");
                        } else {
                            insights_kept += 1;
                            crate::embeddings::embed_and_store(
                                ollama,
                                &memory.embeddings,
                                "cognition",
                                &id.to_string(),
                                &content,
                            )
                            .await;
                            info!("dmn: kept insight — {}", insight.chars().take(80).collect::<String>());
                        }
                    }
                }
            }
        }
        Err(e) => warn!("dmn: wander skipped (ollama down?): {e}"),
    }

    // ── Future simulation: imagine the next few rounds ───────────────────────
    let simulations = simulate_future(memory, ollama, round).await.unwrap_or(0);

    let report = WanderReport {
        fragments_sampled: fragments.len(),
        insights_kept,
        simulations,
    };
    info!(
        "dmn: default mode complete — sampled={} insights={} simulations={}",
        report.fragments_sampled, report.insights_kept, report.simulations
    );
    Ok(report)
}

/// Sample fragments from DIFFERENT memory domains so associations cross
/// boundaries rather than restate one cluster. Uses ORDER BY RANDOM().
fn sample_disparate_fragments(db: &Arc<Mutex<Connection>>, n: usize) -> Result<Vec<String>> {
    let conn = db.lock().unwrap();
    let mut fragments = Vec::new();
    let per_source = (n / 2).max(1);

    // Cognition (knowledge — papers, domain seeds, prior insights)
    if let Ok(mut stmt) =
        conn.prepare("SELECT content FROM cognition ORDER BY RANDOM() LIMIT ?1")
    {
        if let Ok(rows) = stmt.query_map([per_source as i64], |r| r.get::<_, String>(0)) {
            for r in rows.flatten() {
                fragments.push(r);
            }
        }
    }
    // Episodic (lived experience — past task outcomes)
    if let Ok(mut stmt) =
        conn.prepare("SELECT content FROM episodic ORDER BY RANDOM() LIMIT ?1")
    {
        if let Ok(rows) = stmt.query_map([per_source as i64], |r| r.get::<_, String>(0)) {
            for r in rows.flatten() {
                fragments.push(r);
            }
        }
    }
    // Semantic (learned principles)
    if let Ok(mut stmt) =
        conn.prepare("SELECT content FROM semantic ORDER BY RANDOM() LIMIT 2")
    {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for r in rows.flatten() {
                fragments.push(r);
            }
        }
    }
    Ok(fragments)
}

/// Imagine where the next rounds might go, based on the current fingerprint.
/// Stores the most concerning scenario as a cognition item for the Researcher.
async fn simulate_future(
    memory: &Arc<MemoryManager>,
    ollama: &Arc<OllamaClient>,
    round: u32,
) -> Result<usize> {
    use crate::evolved::bf::BfTracker;
    let bf = BfTracker::new(Arc::clone(&memory.db));
    let fp = bf
        .get_round(round.saturating_sub(1))
        .ok()
        .flatten()
        .map(|f| [f.p_tool, f.p_plan, f.p_correct])
        .unwrap_or([0.0, 0.0, 0.0]);

    let prompt = format!(
        "You are imagining your own future — not planning, just simulating. \
         Your current capability fingerprint is [tool={:.2}, planning={:.2}, \
         self_correction={:.2}].\n\n\
         Imagine the most likely concerning scenario for the next 5 rounds: \
         what might stall or regress? Answer in one sentence:\n\
         SCENARIO: <the concerning future>",
        fp[0], fp[1], fp[2],
    );

    match ollama
        .generate(
            &prompt,
            Some("You simulate possible futures. Be concrete and concise."),
            Some(ModelOptions::for_reflection()),
        )
        .await
    {
        Ok(resp) => {
            let (_, answer) = resp.split_thinking();
            if let Some(scenario) = extract_field(&answer, "SCENARIO") {
                if scenario.len() > 15 {
                    let cognition = CognitionStore::new(Arc::clone(&memory.db));
                    let item = CognitionItem::new(
                        format!("[DMN foresight r{round}] {scenario}"),
                        "dmn:simulation".to_string(),
                    );
                    let _ = cognition.insert(&item);
                    return Ok(1);
                }
            }
            Ok(0)
        }
        Err(_) => Ok(0),
    }
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&prefix) {
            let v = rest.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn extract_field_parses_labeled_lines() {
        let text = "INSIGHT: tool retries mirror synaptic pruning\nACTIONABLE: retire after 3 fails";
        assert_eq!(
            extract_field(text, "INSIGHT").as_deref(),
            Some("tool retries mirror synaptic pruning")
        );
        assert_eq!(
            extract_field(text, "ACTIONABLE").as_deref(),
            Some("retire after 3 fails")
        );
        assert!(extract_field(text, "MISSING").is_none());
    }

    #[test]
    fn sample_disparate_pulls_from_multiple_sources() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cognition (id TEXT, content TEXT, source TEXT, keywords TEXT,
                quality REAL, use_count INTEGER, success_count INTEGER, embedding_id INTEGER, created_at TEXT);
             CREATE TABLE episodic (id TEXT, session_id TEXT, task_id TEXT, timestamp TEXT,
                content TEXT, keywords TEXT, importance REAL, embedding_id INTEGER, cluster_id INTEGER);
             CREATE TABLE semantic (id TEXT, content TEXT, source TEXT, keywords TEXT, quality REAL,
                use_count INTEGER, success_count INTEGER, embedding_id INTEGER, cluster_id INTEGER,
                created_at TEXT, last_accessed TEXT);",
        )
        .unwrap();
        for i in 0..3 {
            conn.execute(
                "INSERT INTO cognition (id, content, source, keywords, quality, use_count, success_count, created_at)
                 VALUES (?1, ?2, 'paper', '[]', 0.5, 0, 0, '2026-01-01T00:00:00Z')",
                params![format!("c{i}"), format!("cognition fact {i}")],
            ).unwrap();
            conn.execute(
                "INSERT INTO episodic (id, timestamp, content, keywords, importance)
                 VALUES (?1, '2026-01-01T00:00:00Z', ?2, '[]', 0.5)",
                params![format!("e{i}"), format!("episodic memory {i}")],
            ).unwrap();
        }
        let db = Arc::new(Mutex::new(conn));
        let fragments = sample_disparate_fragments(&db, 6).unwrap();
        assert!(fragments.len() >= 4);
        assert!(fragments.iter().any(|f| f.contains("cognition")));
        assert!(fragments.iter().any(|f| f.contains("episodic")));
    }
}
