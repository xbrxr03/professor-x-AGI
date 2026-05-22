use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Verified skills. Schema from Voyager (arXiv:2305.16291).
/// Stored as SKILL.md-compatible bodies indexed by embedding on description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralEntry {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// Full SKILL.md body or code block.
    pub skill_body: String,
    pub verified: bool,
    pub verification_score: f32,
    pub times_used: u32,
    pub times_succeeded: u32,
    pub embedding_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub source_task_id: Option<Uuid>,
}

impl ProceduralEntry {
    pub fn new(name: String, description: String, skill_body: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            skill_body,
            verified: false,
            verification_score: 0.0,
            times_used: 0,
            times_succeeded: 0,
            embedding_id: None,
            created_at: Utc::now(),
            source_task_id: None,
        }
    }
}

pub struct ProceduralStore {
    db: Arc<Mutex<Connection>>,
}

impl ProceduralStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn upsert(&self, entry: &ProceduralEntry) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO procedural
             (id, name, description, skill_body, verified, verification_score,
              times_used, times_succeeded, embedding_id, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(name) DO UPDATE SET
               description=excluded.description,
               skill_body=excluded.skill_body,
               verified=excluded.verified,
               verification_score=excluded.verification_score",
            params![
                entry.id.to_string(),
                entry.name,
                entry.description,
                entry.skill_body,
                entry.verified as i32,
                entry.verification_score,
                entry.times_used,
                entry.times_succeeded,
                entry.embedding_id,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<ProceduralEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, name, description, skill_body, verified, verification_score,
                    times_used, times_succeeded, embedding_id, created_at
             FROM procedural WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], parse_row)?;
        Ok(rows.next().transpose()?.map(|r| r))
    }

    pub fn list_verified(&self) -> Result<Vec<ProceduralEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, name, description, skill_body, verified, verification_score,
                    times_used, times_succeeded, embedding_id, created_at
             FROM procedural WHERE verified = 1
             ORDER BY verification_score DESC",
        )?;
        let rows = stmt.query_map([], parse_row)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn record_use(&self, name: &str, success: bool) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE procedural SET
                times_used = times_used + 1,
                times_succeeded = times_succeeded + ?1
             WHERE name = ?2",
            params![success as i32, name],
        )?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM procedural WHERE name = ?1", params![name])?;
        Ok(())
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<ProceduralEntry> {
    let id: String = row.get(0)?;
    let created_at: String = row.get(9)?;
    Ok(ProceduralEntry {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        name: row.get(1)?,
        description: row.get(2)?,
        skill_body: row.get(3)?,
        verified: row.get::<_, i32>(4)? != 0,
        verification_score: row.get(5)?,
        times_used: row.get::<_, i64>(6)? as u32,
        times_succeeded: row.get::<_, i64>(7)? as u32,
        embedding_id: row.get(8)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        source_task_id: None,
    })
}
