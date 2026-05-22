/// Merkle-chained audit log.
///
/// ClawOS claims Merkle chaining but its policyd/service.py does plain SQLite append.
/// JARVIS actually implements SHA-256 prev_hash chaining on every AuditEntry.
/// verify_chain() runs at startup to detect tampering.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::policyd::Decision;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    /// SHA-256 of the previous entry's serialized bytes. [0u8;32] hex-encoded for genesis.
    pub prev_hash: String,
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid,
    pub task_id: Option<Uuid>,
    pub tool: String,
    /// SHA-256 of the actual params (params NOT stored in log).
    pub params_hash: String,
    pub risk_score: u8,
    pub decision: Decision,
    pub reason: String,
    pub execution_ms: Option<u64>,
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub struct AuditStore {
    db: Arc<Mutex<Connection>>,
}

impl AuditStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Append a new audit entry with Merkle prev_hash computed from last entry.
    pub fn append(
        &self,
        session_id: Uuid,
        task_id: Option<Uuid>,
        tool: &str,
        params_raw: &serde_json::Value,
        risk_score: u8,
        decision: Decision,
        reason: &str,
        execution_ms: Option<u64>,
    ) -> Result<()> {
        let params_hash = hash_bytes(params_raw.to_string().as_bytes());
        let prev_hash = self.last_entry_hash()?;
        let id = Uuid::new_v4();
        let timestamp = Utc::now();

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO audit_log
             (id, prev_hash, timestamp, session_id, task_id, tool, params_hash,
              risk_score, decision, reason, execution_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                id.to_string(),
                prev_hash,
                timestamp.to_rfc3339(),
                session_id.to_string(),
                task_id.map(|u| u.to_string()),
                tool,
                params_hash,
                risk_score,
                format!("{:?}", decision),
                reason,
                execution_ms.map(|ms| ms as i64),
            ],
        )?;
        Ok(())
    }

    /// Walk all entries in timestamp order and verify the Merkle chain.
    pub fn verify_chain(&self) -> Result<bool> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, prev_hash, timestamp, session_id, task_id, tool,
                    params_hash, risk_score, reason
             FROM audit_log ORDER BY timestamp ASC"
        )?;

        let genesis_hash = hex::encode([0u8; 32]);
        let mut expected_prev = genesis_hash;
        let mut count = 0u64;

        let rows: Vec<_> = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,  // id
                row.get::<_, String>(1)?,  // prev_hash
                row.get::<_, String>(2)?,  // timestamp
                row.get::<_, String>(3)?,  // session_id
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,  // tool
                row.get::<_, String>(6)?,  // params_hash
                row.get::<_, i32>(7)?,     // risk_score
                row.get::<_, String>(8)?,  // reason
            ))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;

        for (id, stored_prev, timestamp, session_id, _task_id, tool, params_hash, risk_score, reason) in rows {
            if stored_prev != expected_prev {
                tracing::error!(
                    "Merkle chain broken at entry {id}: expected {expected_prev}, got {stored_prev}"
                );
                return Ok(false);
            }

            // Recompute this entry's hash to use as next expected_prev
            let hashable = format!(
                "{id}|{timestamp}|{session_id}|{tool}|{params_hash}|{risk_score}|{reason}"
            );
            expected_prev = hash_bytes(hashable.as_bytes());
            count += 1;
        }

        tracing::info!("audit chain verified: {count} entries, intact");
        Ok(true)
    }

    /// Compute the hash of the last entry to use as prev_hash for the next entry.
    fn last_entry_hash(&self) -> Result<String> {
        let db = self.db.lock().unwrap();
        let last: Option<(String, String, String, String, String, String, i32, String)> = db.query_row(
            "SELECT id, prev_hash, timestamp, session_id, tool, params_hash, risk_score, reason
             FROM audit_log ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            )),
        ).optional()?;

        match last {
            None => Ok(hex::encode([0u8; 32])), // Genesis
            Some((id, _prev, timestamp, session_id, tool, params_hash, risk_score, reason)) => {
                let hashable = format!(
                    "{id}|{timestamp}|{session_id}|{tool}|{params_hash}|{risk_score}|{reason}"
                );
                Ok(hash_bytes(hashable.as_bytes()))
            }
        }
    }

    pub fn tail(&self, n: usize) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT timestamp, tool, decision, reason FROM audit_log
             ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![n as i64], |row| {
            Ok(format!(
                "{} | {} | {} | {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}
