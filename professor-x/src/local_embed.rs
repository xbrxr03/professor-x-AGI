//! Local in-process ONNX embeddings (jcode-inspired) — drops the Ollama
//! `nomic-embed-text` network/process dependency for every embed call.
//!
//! Uses `fastembed` (ONNX Runtime) with `nomic-embed-text-v1.5` to stay at
//! **768 dimensions**, matching the existing stored vectors so the embedding
//! store remains compatible. The model is downloaded once on first use; if init
//! or inference fails (no model cache, no network on first run, etc.) the caller
//! transparently falls back to the Ollama HTTP path — embeddings never hard-fail.
//!
//! Loaded lazily behind a OnceLock (model init is ~hundreds of ms; the ONNX
//! session is reused for the process lifetime). Embed calls are serialized
//! through a Mutex — our usage is sequential and the win is removing the network
//! hop, not parallelism.

use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};

static EMBEDDER: OnceLock<Option<Mutex<fastembed::TextEmbedding>>> = OnceLock::new();

fn embedder() -> Option<&'static Mutex<fastembed::TextEmbedding>> {
    EMBEDDER
        .get_or_init(|| {
            use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
            match TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(false),
            ) {
                Ok(m) => {
                    info!("local-embed: nomic-embed-text-v1.5 (768d, ONNX) ready — Ollama embed no longer required");
                    Some(Mutex::new(m))
                }
                Err(e) => {
                    warn!("local-embed: init failed ({e}); falling back to Ollama HTTP embeddings");
                    None
                }
            }
        })
        .as_ref()
}

/// True if the local embedder loaded successfully.
pub fn available() -> bool {
    embedder().is_some()
}

/// Embed one text locally (768-dim). None on any failure → caller falls back.
pub fn embed_one(text: &str) -> Option<Vec<f32>> {
    let m = embedder()?;
    let mut guard = m.lock().ok()?;
    let out = guard.embed(vec![text.to_string()], None).ok()?;
    out.into_iter().next()
}

/// Embed a batch locally (768-dim each). None on any failure → caller falls back.
pub fn embed_many(texts: &[&str]) -> Option<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Some(Vec::new());
    }
    let m = embedder()?;
    let mut guard = m.lock().ok()?;
    let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
    guard.embed(owned, None).ok()
}
