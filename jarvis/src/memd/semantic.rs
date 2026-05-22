use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Learned concepts and stable knowledge.
/// Quality formula from EvolveR (arXiv:2510.16079): (success+1)/(use+2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEntry {
    pub id: Uuid,
    pub content: String,
    pub source: String,
    pub keywords: Vec<String>,
    pub quality: f32,
    pub use_count: u32,
    pub success_count: u32,
    pub embedding_id: Option<i64>,
    pub cluster_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

impl SemanticEntry {
    pub fn new(content: String, source: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content,
            source,
            keywords: Vec::new(),
            quality: 0.5,
            use_count: 0,
            success_count: 0,
            embedding_id: None,
            cluster_id: None,
            created_at: now,
            last_accessed: now,
        }
    }

    /// EvolveR quality formula: (success_count+1)/(use_count+2)
    pub fn recompute_quality(&mut self) {
        self.quality = (self.success_count as f32 + 1.0) / (self.use_count as f32 + 2.0);
    }
}

pub struct SemanticStore {
    db: Arc<Mutex<Connection>>,
}

impl SemanticStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, entry: &SemanticEntry) -> Result<()> {
        let db = self.db.lock().unwrap();
        let keywords_json = serde_json::to_string(&entry.keywords)?;
        db.execute(
            "INSERT OR IGNORE INTO semantic
             (id, content, source, keywords, quality, use_count, success_count,
              embedding_id, cluster_id, created_at, last_accessed)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                entry.id.to_string(),
                entry.content,
                entry.source,
                keywords_json,
                entry.quality,
                entry.use_count,
                entry.success_count,
                entry.embedding_id,
                entry.cluster_id,
                entry.created_at.to_rfc3339(),
                entry.last_accessed.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn record_use(&self, id: &Uuid, success: bool) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE semantic SET
                use_count = use_count + 1,
                success_count = success_count + ?1,
                quality = (success_count + ?1 + 1.0) / (use_count + 1 + 2.0),
                last_accessed = ?2
             WHERE id = ?3",
            params![
                success as i32,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn search_keywords(&self, keywords: &[String], limit: usize) -> Result<Vec<SemanticEntry>> {
        let db = self.db.lock().unwrap();
        // Simple keyword overlap via LIKE — FTS5 for semantic table is not set up yet.
        // Replace with FAISS cosine search once embeddings are active.
        if keywords.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", keywords[0]);
        let mut stmt = db.prepare(
            "SELECT id, content, source, keywords, quality, use_count, success_count,
                    embedding_id, cluster_id, created_at, last_accessed
             FROM semantic WHERE content LIKE ?1
             ORDER BY quality DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<SemanticEntry> {
    let id: String = row.get(0)?;
    let keywords_json: String = row.get(3)?;
    let created_at: String = row.get(9)?;
    let last_accessed: String = row.get(10)?;

    Ok(SemanticEntry {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        content: row.get(1)?,
        source: row.get(2)?,
        keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
        quality: row.get(4)?,
        use_count: row.get::<_, i64>(5)? as u32,
        success_count: row.get::<_, i64>(6)? as u32,
        embedding_id: row.get(7)?,
        cluster_id: row.get(8)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_accessed: DateTime::parse_from_rfc3339(&last_accessed)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}
