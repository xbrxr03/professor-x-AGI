/// Cognition base — accumulated knowledge injected into each evolution cycle.
/// Mirrors ASI-Evolve CognitionItem dataclass (utils/structures.py).
/// Quality formula from EvolveR (arXiv:2510.16079): (success+1)/(use+2)

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitionItem {
    pub id: Uuid,
    pub content: String,
    /// "paper", "experiment", "reflection"
    pub source: String,
    pub keywords: Vec<String>,
    /// EvolveR quality: (success_count+1)/(use_count+2)
    pub quality: f32,
    pub use_count: u32,
    pub success_count: u32,
    pub embedding_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl CognitionItem {
    pub fn new(content: String, source: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            source,
            keywords: Vec::new(),
            quality: 0.5,
            use_count: 0,
            success_count: 0,
            embedding_id: None,
            created_at: Utc::now(),
        }
    }

    pub fn recompute_quality(&mut self) {
        self.quality = (self.success_count as f32 + 1.0) / (self.use_count as f32 + 2.0);
    }
}

pub struct CognitionStore {
    db: Arc<Mutex<Connection>>,
}

impl CognitionStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, item: &CognitionItem) -> Result<()> {
        let db = self.db.lock().unwrap();
        let keywords_json = serde_json::to_string(&item.keywords)?;
        db.execute(
            "INSERT OR IGNORE INTO cognition
             (id, content, source, keywords, quality, use_count, success_count, embedding_id, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                item.id.to_string(),
                item.content,
                item.source,
                keywords_json,
                item.quality,
                item.use_count,
                item.success_count,
                item.embedding_id,
                item.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Seed from paper summaries (~100 items) — called at first startup.
    pub fn seed_if_empty(&self, items: Vec<CognitionItem>) -> Result<()> {
        let db = self.db.lock().unwrap();
        let count: i64 = db.query_row("SELECT COUNT(*) FROM cognition", [], |r| r.get(0))?;
        drop(db);

        if count == 0 {
            tracing::info!("seeding cognition base with {} items", items.len());
            for item in items {
                self.insert(&item)?;
            }
        }
        Ok(())
    }

    /// Semantic retrieval via pre-computed embeddings (nomic-embed-text).
    /// Falls back to empty if no cognition embeddings are stored yet.
    pub fn search_semantic(
        &self,
        emb_store: &crate::embeddings::EmbeddingStore,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<CognitionItem>> {
        let top = emb_store.top_k("cognition", query_vec, k)?;
        if top.is_empty() {
            return Ok(Vec::new());
        }
        let db = self.db.lock().unwrap();
        let mut results = Vec::new();
        for (source_id, _sim) in top {
            let mut stmt = db.prepare(
                "SELECT id, content, source, keywords, quality, use_count, success_count,
                        embedding_id, created_at
                 FROM cognition WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map(params![source_id], parse_item)?;
            if let Some(row) = rows.next() {
                results.push(row?);
            }
        }
        Ok(results)
    }

    /// Retrieve top-k items by keyword relevance (FTS fallback until embeddings active).
    pub fn query_top_k(&self, query: &str, k: usize) -> Result<Vec<CognitionItem>> {
        let db = self.db.lock().unwrap();
        let pattern = format!("%{query}%");
        let mut stmt = db.prepare(
            "SELECT id, content, source, keywords, quality, use_count, success_count,
                    embedding_id, created_at
             FROM cognition WHERE content LIKE ?1
             ORDER BY quality DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, k as i64], parse_item)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn count(&self) -> Result<i64> {
        let db = self.db.lock().unwrap();
        Ok(db.query_row("SELECT COUNT(*) FROM cognition", [], |r| r.get(0))?)
    }

    /// All cognition items — used to backfill embeddings and map ids to content.
    pub fn all(&self) -> Result<Vec<CognitionItem>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, content, source, keywords, quality, use_count, success_count,
                    embedding_id, created_at
             FROM cognition",
        )?;
        let rows = stmt.query_map([], parse_item)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn record_use(&self, id: &Uuid, success: bool) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE cognition SET
                use_count = use_count + 1,
                success_count = success_count + ?1,
                quality = (success_count + ?1 + 1.0) / (use_count + 1 + 2.0)
             WHERE id = ?2",
            params![success as i32, id.to_string()],
        )?;
        Ok(())
    }
}

fn parse_item(row: &rusqlite::Row) -> rusqlite::Result<CognitionItem> {
    let id: String = row.get(0)?;
    let keywords_json: String = row.get(3)?;
    let created_at: String = row.get(8)?;
    Ok(CognitionItem {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        content: row.get(1)?,
        source: row.get(2)?,
        keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
        quality: row.get(4)?,
        use_count: row.get::<_, i64>(5)? as u32,
        success_count: row.get::<_, i64>(6)? as u32,
        embedding_id: row.get(7)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}
