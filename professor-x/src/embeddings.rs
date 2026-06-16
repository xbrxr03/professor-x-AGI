/// Embedding store — SQLite-backed dense vector storage with brute-force cosine search.
///
/// Uses Ollama's `nomic-embed-text` model (768-dim, CPU-only, ~274MB).
/// Vectors are stored as raw f32 LE bytes in a BLOB column.
///
/// No FAISS required at this scale (< 10K entries, brute-force < 2ms).
/// FAISS can be wired in later if the store grows beyond ~50K entries.
///
/// Run once to enable: `ollama pull nomic-embed-text`
use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use tracing::warn;

// ── Math ──────────────────────────────────────────────────────────────────────

/// Cosine similarity ∈ [-1, 1]. Returns 0.0 on zero-norm or length mismatch.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

fn vec_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn bytes_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ── Store ─────────────────────────────────────────────────────────────────────

/// SQLite-backed vector store. One row = one embedding keyed by (source_table, source_id).
#[derive(Clone)]
pub struct EmbeddingStore {
    db: Arc<Mutex<Connection>>,
}

impl EmbeddingStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    /// Insert or replace the embedding for a source row.
    pub fn upsert(&self, source_table: &str, source_id: &str, vector: &[f32]) -> Result<()> {
        let bytes = vec_to_bytes(vector);
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO embeddings (source_table, source_id, vector)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(source_table, source_id) DO UPDATE SET vector = excluded.vector",
            params![source_table, source_id, bytes],
        )?;
        Ok(())
    }

    /// Fetch a single vector.
    pub fn get(&self, source_table: &str, source_id: &str) -> Result<Option<Vec<f32>>> {
        let db = self.db.lock().unwrap();
        let result = db.query_row(
            "SELECT vector FROM embeddings WHERE source_table = ?1 AND source_id = ?2",
            params![source_table, source_id],
            |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(bytes) => Ok(Some(bytes_to_vec(&bytes))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Fetch all (source_id, vector) pairs for a table. Used for brute-force search.
    pub fn all_for(&self, source_table: &str) -> Result<Vec<(String, Vec<f32>)>> {
        let db = self.db.lock().unwrap();
        let mut stmt =
            db.prepare("SELECT source_id, vector FROM embeddings WHERE source_table = ?1")?;
        let rows = stmt.query_map(params![source_table], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (id, bytes) = row?;
            result.push((id, bytes_to_vec(&bytes)));
        }
        Ok(result)
    }

    /// Return the top-k source_ids most similar to `query`, sorted descending.
    pub fn top_k(&self, source_table: &str, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let candidates = self.all_for(source_table)?;
        let mut scored: Vec<(String, f32)> = candidates
            .into_iter()
            .map(|(id, vec)| {
                let sim = cosine_similarity(query, &vec);
                (id, sim)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        Ok(scored)
    }
}

// ── Async helper ─────────────────────────────────────────────────────────────

/// Embed text via Ollama and store it. Returns the vector on success, None on failure.
/// Failures are logged as warnings — never propagated so callers always continue.
pub async fn embed_and_store(
    ollama: &crate::ollama::OllamaClient,
    store: &EmbeddingStore,
    source_table: &str,
    source_id: &str,
    text: &str,
) -> Option<Vec<f32>> {
    match ollama.embed(text).await {
        Ok(vec) => {
            if let Err(e) = store.upsert(source_table, source_id, &vec) {
                warn!("embeddings: failed to store vector for {source_table}/{source_id}: {e}");
            }
            Some(vec)
        }
        Err(e) => {
            // nomic-embed-text not pulled yet → silent degradation to FTS5
            warn!("embeddings: embed failed (is nomic-embed-text pulled?): {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0f32, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn bytes_roundtrip() {
        let v = vec![1.5f32, -0.3, 0.0, 42.0];
        let b = vec_to_bytes(&v);
        let v2 = bytes_to_vec(&b);
        for (a, b) in v.iter().zip(v2.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn store_upsert_and_top_k() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_table TEXT NOT NULL,
                source_id TEXT NOT NULL,
                vector BLOB NOT NULL,
                UNIQUE(source_table, source_id)
            );",
        )
        .unwrap();
        let store = EmbeddingStore::new(Arc::new(Mutex::new(conn)));

        store.upsert("episodic", "a", &[1.0, 0.0]).unwrap();
        store.upsert("episodic", "b", &[0.0, 1.0]).unwrap();
        store.upsert("episodic", "c", &[0.9, 0.1]).unwrap();

        let top = store.top_k("episodic", &[1.0, 0.0], 2).unwrap();
        assert_eq!(top[0].0, "a");
        assert_eq!(top[1].0, "c");
    }
}
