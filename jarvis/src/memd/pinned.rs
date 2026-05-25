use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedEntry {
    pub id: String,
    pub content: String,
    /// If true, the evolved module cannot modify this entry.
    pub immutable: bool,
}

pub struct PinnedStore {
    db: Arc<Mutex<Connection>>,
}

impl PinnedStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn upsert(&self, entry: &PinnedEntry) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO pinned (id, content, immutable) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET content=excluded.content, immutable=excluded.immutable",
            params![entry.id, entry.content, entry.immutable as i32],
        )?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<PinnedEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare("SELECT id, content, immutable FROM pinned ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(PinnedEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                immutable: row.get::<_, i32>(2)? != 0,
            })
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM pinned WHERE id = ?1 AND immutable = 0", params![id])?;
        Ok(())
    }
}
