use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Past session memory. Schema derived from Hermes Agent sessions/messages tables.
/// Retrieval scoring from arXiv:2603.07670 (Memory for Autonomous LLM Agents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicEntry {
    pub id: Uuid,
    pub session_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub content: String,
    pub keywords: Vec<String>,
    /// Self-assessed importance 0.0–1.0.
    pub importance: f32,
    /// Row ID in episodic.faiss index (None until embedding is computed).
    pub embedding_id: Option<i64>,
    pub cluster_id: Option<i32>,
}

pub struct EpisodicStore {
    db: Arc<Mutex<Connection>>,
}

impl EpisodicStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Write pipeline: caller is responsible for running filter→tag→dedupe→score first.
    pub fn insert(&self, entry: &EpisodicEntry) -> Result<()> {
        let db = self.db.lock().unwrap();
        let keywords_json = serde_json::to_string(&entry.keywords)?;
        db.execute(
            "INSERT OR IGNORE INTO episodic
             (id, session_id, task_id, timestamp, content, keywords, importance, embedding_id, cluster_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id.to_string(),
                entry.session_id.map(|u| u.to_string()),
                entry.task_id.map(|u| u.to_string()),
                entry.timestamp.to_rfc3339(),
                entry.content,
                keywords_json,
                entry.importance,
                entry.embedding_id,
                entry.cluster_id,
            ],
        )?;
        Ok(())
    }

    /// Multi-signal retrieval scoring (arXiv:2603.07670):
    /// score = α·cosine + β·recency + γ·importance
    /// Default: α=0.5, β=0.3, γ=0.2, λ=0.1
    ///
    /// Without embeddings, falls back to FTS5 keyword search.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<EpisodicEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT e.id, e.session_id, e.task_id, e.timestamp, e.content,
                    e.keywords, e.importance, e.embedding_id, e.cluster_id
             FROM episodic e
             JOIN episodic_fts fts ON e.rowid = fts.rowid
             WHERE episodic_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, limit as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn recent(&self, n: usize) -> Result<Vec<EpisodicEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, session_id, task_id, timestamp, content,
                    keywords, importance, embedding_id, cluster_id
             FROM episodic ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![n as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<EpisodicEntry> {
    let id: String = row.get(0)?;
    let session_id: Option<String> = row.get(1)?;
    let task_id: Option<String> = row.get(2)?;
    let timestamp: String = row.get(3)?;
    let keywords_json: String = row.get(5)?;

    Ok(EpisodicEntry {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        session_id: session_id.and_then(|s| Uuid::parse_str(&s).ok()),
        task_id: task_id.and_then(|s| Uuid::parse_str(&s).ok()),
        timestamp: DateTime::parse_from_rfc3339(&timestamp)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        content: row.get(4)?,
        keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
        importance: row.get(6)?,
        embedding_id: row.get(7)?,
        cluster_id: row.get(8)?,
    })
}
