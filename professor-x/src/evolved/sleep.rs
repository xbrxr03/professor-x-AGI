/// Sleep Consolidation (Seed 3 — Complementary Learning Systems).
///
/// McClelland, McNaughton & O'Reilly (1995): you cannot have one memory system
/// that does everything. You need two that interact. The hippocampus encodes
/// specific experiences fast (one-shot, pattern-separated, decaying). The
/// neocortex extracts statistical regularities slowly (many repetitions,
/// pattern-completed, permanent). During sleep, the hippocampus REPLAYS
/// memories to the neocortex — compressed and reordered to highlight what's
/// important. The neocortex learns the statistics offline.
///
/// For Professor X:
///   - episodic store = hippocampus (fast, decaying)
///   - semantic store = neocortex (slow, permanent)
///
/// `consolidate` runs after each evolution cycle (the agent's "sleep"):
///   1. Replay: gather recent episodic memories
///   2. MARS-at-scale: extract cross-task statistical patterns via one LLM pass
///   3. Promote: write recurring patterns to semantic memory (the neocortex learns)
///   4. Decay: Ebbinghaus forgetting — old, low-importance episodics are pruned
///
/// This compounds learning across days. Without it, each round starts from the
/// same raw episodic soup. With it, each round starts from a cleaner, more
/// condensed memory that has already extracted what matters.

use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tracing::{info, warn};

use crate::memd::semantic::SemanticEntry;
use crate::memd::MemoryManager;
use crate::ollama::{ModelOptions, OllamaClient};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsolidationReport {
    pub episodics_replayed: usize,
    pub patterns_extracted: usize,
    pub promoted_to_semantic: usize,
    pub episodics_decayed: usize,
}

/// Run one sleep-consolidation cycle. Safe to call with Ollama down — it will
/// skip the LLM-dependent extraction and still perform decay.
pub async fn consolidate(
    memory: &Arc<MemoryManager>,
    ollama: &Arc<OllamaClient>,
    round: u32,
) -> Result<ConsolidationReport> {
    info!("sleep: beginning consolidation for round {round}");

    // ── 1. Replay: gather recent episodic memories ───────────────────────────
    let recent = memory.episodic.recent(40).unwrap_or_default();
    let replayed = recent.len();
    if replayed == 0 {
        info!("sleep: no episodic memories to consolidate");
        return Ok(ConsolidationReport {
            episodics_replayed: 0,
            patterns_extracted: 0,
            promoted_to_semantic: 0,
            episodics_decayed: 0,
        });
    }

    // ── 2. MARS-at-scale: extract patterns across ALL recent failures at once ─
    // The brain doesn't reflect on failures one at a time during sleep — it
    // replays them in batches and extracts what's common. One LLM pass over
    // the whole batch finds cross-task regularities a per-task reflection
    // cannot see.
    let mut promoted = 0;
    let mut patterns_extracted = 0;

    let episodic_digest = recent
        .iter()
        .map(|e| format!("- [{}] {}", e.importance, e.content))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are consolidating memory during sleep. Below are recent task \
         experiences. Find the 1-3 STATISTICAL PATTERNS that recur across \
         multiple experiences — not one-off events, but regularities that \
         would help on future tasks.\n\n\
         Experiences:\n{episodic_digest}\n\n\
         For each pattern, output one line:\n\
         PATTERN: <a general rule extracted from multiple experiences>\n\
         Only output patterns that appear in 2+ experiences. If none recur, output nothing.",
    );

    match ollama
        .generate(
            &prompt,
            Some("You are a memory consolidation process. Extract only recurring statistical patterns."),
            Some(ModelOptions::for_reflection()),
        )
        .await
    {
        Ok(resp) => {
            let (_, answer) = resp.split_thinking();
            for line in answer.lines() {
                let trimmed = line.trim();
                if let Some(pattern) = trimmed.strip_prefix("PATTERN:") {
                    let pattern = pattern.trim();
                    if pattern.len() < 15 {
                        continue;
                    }
                    patterns_extracted += 1;
                    // ── 3. Promote: the neocortex learns the pattern ──────────
                    let entry = SemanticEntry::new(
                        format!("[consolidated r{round}] {pattern}"),
                        "sleep:consolidation".to_string(),
                    );
                    let id = entry.id;
                    if let Err(e) = memory.semantic.insert(&entry) {
                        warn!("sleep: failed to promote pattern to semantic: {e}");
                    } else {
                        promoted += 1;
                        // Embed for future semantic retrieval (best-effort)
                        crate::embeddings::embed_and_store(
                            ollama,
                            &memory.embeddings,
                            "semantic",
                            &id.to_string(),
                            pattern,
                        )
                        .await;
                    }
                }
            }
        }
        Err(e) => warn!("sleep: MARS-at-scale extraction skipped (ollama down?): {e}"),
    }

    // ── 4. Decay: Ebbinghaus forgetting curve ────────────────────────────────
    // Episodic memories that are old AND low-importance are pruned. This is
    // not data loss — the important patterns have already been promoted to
    // semantic memory (the neocortex). The hippocampus clears space.
    let decayed = decay_stale_episodics(memory)?;

    let report = ConsolidationReport {
        episodics_replayed: replayed,
        patterns_extracted,
        promoted_to_semantic: promoted,
        episodics_decayed: decayed,
    };
    info!(
        "sleep: consolidation complete — replayed={} patterns={} promoted={} decayed={}",
        report.episodics_replayed,
        report.patterns_extracted,
        report.promoted_to_semantic,
        report.episodics_decayed
    );
    Ok(report)
}

/// Ebbinghaus decay: prune episodic entries older than 7 days with
/// importance below 0.5. Mirrors the hippocampal ~7-day retention window
/// before consolidation; salient (high-importance) memories are spared.
fn decay_stale_episodics(memory: &Arc<MemoryManager>) -> Result<usize> {
    let cutoff = (Utc::now() - Duration::days(7)).to_rfc3339();
    let db = memory.db.lock().unwrap();
    let deleted = db.execute(
        "DELETE FROM episodic
         WHERE timestamp < ?1 AND importance < 0.5",
        rusqlite::params![cutoff],
    )?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memd::episodic::EpisodicEntry;
    use chrono::Utc;
    use uuid::Uuid;

    fn mem() -> Arc<MemoryManager> {
        let dir = std::env::temp_dir().join(format!("px-sleep-{}", Uuid::new_v4()));
        Arc::new(MemoryManager::open(&dir).unwrap())
    }

    #[test]
    fn decay_removes_old_low_importance_only() {
        let memory = mem();
        let old_low = EpisodicEntry {
            id: Uuid::new_v4(),
            session_id: None,
            task_id: None,
            timestamp: Utc::now() - Duration::days(10),
            content: "old unimportant failure".to_string(),
            keywords: vec![],
            importance: 0.3,
            embedding_id: None,
            cluster_id: None,
        };
        let old_high = EpisodicEntry {
            id: Uuid::new_v4(),
            session_id: None,
            task_id: None,
            timestamp: Utc::now() - Duration::days(10),
            content: "old important success".to_string(),
            keywords: vec![],
            importance: 0.9,
            embedding_id: None,
            cluster_id: None,
        };
        let fresh = EpisodicEntry {
            id: Uuid::new_v4(),
            session_id: None,
            task_id: None,
            timestamp: Utc::now(),
            content: "recent failure".to_string(),
            keywords: vec![],
            importance: 0.2,
            embedding_id: None,
            cluster_id: None,
        };
        memory.episodic.insert(&old_low).unwrap();
        memory.episodic.insert(&old_high).unwrap();
        memory.episodic.insert(&fresh).unwrap();

        let decayed = decay_stale_episodics(&memory).unwrap();
        assert_eq!(decayed, 1); // only old_low

        let remaining = memory.episodic.recent(10).unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|e| e.content != "old unimportant failure"));
    }
}
