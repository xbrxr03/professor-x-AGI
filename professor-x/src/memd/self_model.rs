//! Strange Loop self-model storage (paper outline §4.7).
//!
//! Professor X's evolving self-description, persisted as a snapshot every
//! N rounds (10 in the paper plan). The round-0 seed lives in
//! `personas/professor_x.md`; later snapshots are LLM-generated from
//! behavioural fingerprint history, MCA, mean affect, and the prior
//! self-description. ICS (`memd::ics`) measures how much the snapshot has
//! drifted from the round-0 baseline.
//!
//! Lineage:
//! - Hofstadter, "I Am a Strange Loop" — the self is a self-referential
//!   pattern with downward causation. We operationalise the pattern as the
//!   sequence of `text` fields and the embedding similarity between them.
//! - arXiv:2506.05109 — calls for a metacognitive self-model. This module
//!   is one of the implementation surfaces.
//!
//! **This file is a stub.** The struct + persistence layer are real;
//! `update_via_llm` is a TODO that needs the prompt design + Ollama hookup
//! before it can be called in production.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Snapshot of Professor X's self-description at a specific round.
/// The text is the canonical artefact ICS compares; the embedding_id, when
/// set, points into the embeddings store for fast cosine lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModelSnapshot {
    pub id: Option<i64>,
    pub round: u32,
    pub text: String,
    pub embedding_id: Option<i64>,
    pub recorded_at: DateTime<Utc>,
}

impl SelfModelSnapshot {
    pub fn new(round: u32, text: impl Into<String>) -> Self {
        Self {
            id: None,
            round,
            text: text.into(),
            embedding_id: None,
            recorded_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct SelfModelStore {
    db: Arc<Mutex<Connection>>,
}

impl SelfModelStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn append(&self, snapshot: &SelfModelSnapshot) -> Result<i64> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO self_model (round, text, embedding_id, recorded_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot.round as i64,
                snapshot.text,
                snapshot.embedding_id,
                snapshot.recorded_at.to_rfc3339(),
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Round-0 seed loader. Reads `personas/professor_x.md` from the
    /// workspace root and persists it if no snapshot exists yet. Idempotent.
    pub fn seed_if_empty(&self, seed_text: impl Into<String>) -> Result<()> {
        if self.latest()?.is_some() {
            return Ok(());
        }
        let snap = SelfModelSnapshot::new(0, seed_text);
        self.append(&snap)?;
        Ok(())
    }

    pub fn latest(&self) -> Result<Option<SelfModelSnapshot>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, text, embedding_id, recorded_at
             FROM self_model
             ORDER BY id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], parse_row)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn at_round(&self, round: u32) -> Result<Option<SelfModelSnapshot>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, text, embedding_id, recorded_at
             FROM self_model
             WHERE round = ?1
             ORDER BY id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![round as i64], parse_row)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// All snapshots, oldest first. For ICS trajectory plots.
    pub fn history(&self) -> Result<Vec<SelfModelSnapshot>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, round, text, embedding_id, recorded_at
             FROM self_model
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// TODO: build the update prompt (prior_text + fingerprint history +
    /// MCA + affect summary), call the Ollama generate API, parse the
    /// response, embed it, and persist. Wired by the daily-cycle at the
    /// 10-round boundary.
    pub fn update_via_llm(&self, _round: u32) -> Result<Option<SelfModelSnapshot>> {
        Ok(None)
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<SelfModelSnapshot> {
    let recorded_at: String = row.get(4)?;
    Ok(SelfModelSnapshot {
        id: Some(row.get(0)?),
        round: row.get::<_, i64>(1)? as u32,
        text: row.get(2)?,
        embedding_id: row.get(3)?,
        recorded_at: DateTime::parse_from_rfc3339(&recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_store() -> SelfModelStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE self_model (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                round INTEGER NOT NULL,
                text TEXT NOT NULL,
                embedding_id INTEGER,
                recorded_at TEXT NOT NULL
            );",
        )
        .unwrap();
        SelfModelStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn seed_if_empty_is_idempotent() {
        let store = fresh_store();
        store.seed_if_empty("I am Professor X.").unwrap();
        store.seed_if_empty("Different text, should not overwrite.").unwrap();
        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.text, "I am Professor X.");
        assert_eq!(latest.round, 0);
    }

    #[test]
    fn history_orders_oldest_first() {
        let store = fresh_store();
        store.append(&SelfModelSnapshot::new(0, "a")).unwrap();
        store.append(&SelfModelSnapshot::new(10, "b")).unwrap();
        store.append(&SelfModelSnapshot::new(20, "c")).unwrap();
        let h = store.history().unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].text, "a");
        assert_eq!(h[2].text, "c");
    }

    #[test]
    fn at_round_returns_matching_snapshot() {
        let store = fresh_store();
        store.append(&SelfModelSnapshot::new(0, "a")).unwrap();
        store.append(&SelfModelSnapshot::new(10, "b")).unwrap();
        let at_10 = store.at_round(10).unwrap().unwrap();
        assert_eq!(at_10.text, "b");
        assert!(store.at_round(99).unwrap().is_none());
    }
}
