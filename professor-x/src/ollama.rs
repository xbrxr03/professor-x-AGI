/// Ollama HTTP client.
///
/// All LLM inference routes through here. Implements:
/// - generate() for single-turn completions (Thought generation in ReAct)
/// - chat() for multi-turn with system prompt
/// - health_check() / is_model_loaded()
/// - Retry with exponential backoff (Hermes pattern)
///
/// Model default: qwen3:8b-q4_k_m (5.2GB VRAM, 42 tok/s, thinking mode)
/// Thinking mode: set "think" in options to enable <think>...</think> prefix

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::warn;

pub const DEFAULT_MODEL: &str = "qwen3:8b-q4_k_m";
/// Dedicated embedding model — much smaller than the main LLM, CPU-only.
/// Run: `ollama pull nomic-embed-text`
pub const EMBED_MODEL: &str = "nomic-embed-text";
pub const MAX_RETRIES: u32 = 4;
pub const RETRY_BASE_MS: u64 = 500;

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ModelOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<i64>>,
    /// Base64-encoded images for multimodal models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    /// Qwen3 thinking mode — TOP-LEVEL request field (NOT an option). Ollama
    /// rejects `think` inside `options`. false = no <think> block (faster,
    /// better ReAct format compliance).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Qwen3 thinking-mode carrier. NEVER serialized into `options` — Ollama
    /// rejects it there ("invalid option provided"). The request builders lift
    /// this to the top-level `think` field of the request. Kept here so call
    /// sites can express intent in one place (e.g. `for_react` disables it).
    #[serde(skip_serializing)]
    pub think: Option<bool>,
}

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            num_ctx: Some(32768),
            top_p: Some(0.9),
            stop: None,
            think: None,
        }
    }
}

impl ModelOptions {
    pub fn for_react() -> Self {
        Self {
            temperature: Some(0.3),
            num_ctx: Some(16384),
            top_p: Some(0.9),
            stop: Some(vec!["Observation:".to_string()]),
            think: Some(false), // disable thinking — hurts format compliance in tight ReAct loop
        }
    }

    pub fn for_reflection() -> Self {
        Self {
            temperature: Some(0.5),
            num_ctx: Some(8192),
            top_p: Some(0.95),
            stop: None,
            think: Some(true),
        }
    }

    pub fn for_evolution() -> Self {
        Self {
            temperature: Some(0.4),
            num_ctx: Some(32768),
            top_p: Some(0.9),
            stop: None,
            think: Some(true),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
    #[serde(default)]
    pub context: Vec<i64>,
    pub prompt_eval_count: Option<u32>,
    pub eval_count: Option<u32>,
    pub eval_duration: Option<u64>,
}

impl GenerateResponse {
    pub fn tokens_used(&self) -> u32 {
        self.prompt_eval_count.unwrap_or(0) + self.eval_count.unwrap_or(0)
    }

    pub fn tok_per_sec(&self) -> f32 {
        match (self.eval_count, self.eval_duration) {
            (Some(toks), Some(ns)) if ns > 0 => toks as f32 / (ns as f32 / 1e9),
            _ => 0.0,
        }
    }

    /// Strip <think>...</think> block that Qwen3 emits in thinking mode.
    /// Returns (thinking: Option<String>, answer: String)
    pub fn split_thinking(&self) -> (Option<String>, String) {
        let resp = self.response.as_str();
        if let Some(start) = resp.find("<think>") {
            if let Some(end) = resp.find("</think>") {
                let thinking = resp[start + 7..end].trim().to_string();
                let answer = resp[end + 8..].trim().to_string();
                return (Some(thinking), answer);
            }
        }
        (None, resp.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ModelOptions>,
    /// Top-level thinking-mode field (see GenerateRequest::think).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub model: String,
    pub message: ChatMessage,
    pub done: bool,
    pub prompt_eval_count: Option<u32>,
    pub eval_count: Option<u32>,
}

impl ChatResponse {
    pub fn tokens_used(&self) -> u32 {
        self.prompt_eval_count.unwrap_or(0) + self.eval_count.unwrap_or(0)
    }

    pub fn split_thinking(&self) -> (Option<String>, String) {
        let content = &self.message.content;
        if let Some(start) = content.find("<think>") {
            if let Some(end) = content.find("</think>") {
                let thinking = content[start + 7..end].trim().to_string();
                let answer = content[end + 8..].trim().to_string();
                return (Some(thinking), answer);
            }
        }
        (None, content.clone())
    }
}

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    details: Option<ModelDetails>,
}

#[derive(Debug, Deserialize)]
struct ModelDetails {
    #[serde(default)]
    parameter_size: Option<String>,
}

/// An installed Ollama model with its rough scale, for VRAM-aware selection.
#[derive(Debug, Clone)]
pub struct InstalledModel {
    pub name: String,
    pub params_b: f32,
    pub size_bytes: u64,
}

/// Parse a parameter-size string like "8.2B" / "32B" / "1.5B" into billions.
fn parse_params_b(s: &str) -> Option<f32> {
    let t = s.trim().trim_end_matches(|c| c == 'B' || c == 'b').trim();
    t.parse::<f32>().ok()
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct OllamaClient {
    base_url: String,
    http: reqwest::Client,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min for long completions
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client build");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
            model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Check if Ollama is reachable and our model is loaded.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self.http.get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(false);
        }

        let tags: TagsResponse = resp.json().await?;
        let model_base = self.model.split(':').next().unwrap_or(&self.model);
        let loaded = tags.models.iter().any(|m| {
            m.name == self.model
                || m.name.starts_with(model_base)
                || m.name.trim_end_matches(":latest") == self.model.trim_end_matches(":latest")
        });
        Ok(loaded)
    }

    /// List installed models with their rough scale (excludes embedding models).
    pub async fn installed_models(&self) -> Result<Vec<InstalledModel>> {
        let url = format!("{}/api/tags", self.base_url);
        let tags: TagsResponse = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?
            .json()
            .await?;
        Ok(tags
            .models
            .into_iter()
            .filter(|m| !m.name.to_lowercase().contains("embed"))
            .map(|m| {
                let params_b = m
                    .details
                    .as_ref()
                    .and_then(|d| d.parameter_size.as_deref())
                    .and_then(parse_params_b)
                    .unwrap_or(0.0);
                InstalledModel { name: m.name, params_b, size_bytes: m.size }
            })
            .collect())
    }

    /// Pick the largest installed chat model. VRAM-aware by proxy: users only
    /// pull models their hardware can actually run, so "biggest installed" is
    /// "best the local machine can do." Professor X is one harness that scales
    /// across the whole local-model spectrum — 8B on a laptop, 70B on a workstation.
    pub async fn best_local_model(&self) -> Option<String> {
        let mut models = self.installed_models().await.ok()?;
        if models.is_empty() {
            return None;
        }
        models.sort_by(|a, b| {
            b.params_b
                .partial_cmp(&a.params_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.size_bytes.cmp(&a.size_bytes))
        });
        models.into_iter().next().map(|m| m.name)
    }

    /// Single-turn generate. Used for ReAct Thought generation.
    /// Retries up to MAX_RETRIES with exponential backoff.
    pub async fn generate(
        &self,
        prompt: &str,
        system: Option<&str>,
        options: Option<ModelOptions>,
    ) -> Result<GenerateResponse> {
        let think = options.as_ref().and_then(|o| o.think);
        let req = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(str::to_string),
            options,
            context: None,
            images: None,
            think,
        };

        self.generate_with_retry(&req).await
    }

    /// Multi-turn chat. Used for Reflexion and MARS reflection.
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        options: Option<ModelOptions>,
    ) -> Result<ChatResponse> {
        let think = options.as_ref().and_then(|o| o.think);
        let req = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            options,
            think,
        };
        self.chat_with_retry(&req).await
    }

    /// Embed a single text using `nomic-embed-text` (768-dim).
    /// Requires: `ollama pull nomic-embed-text`
    /// Falls back gracefully — callers should treat Err as "embedding unavailable".
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Local ONNX embeddings first (no network/process hop, 768-dim). Falls
        // through to the Ollama HTTP path if the local embedder is unavailable.
        let owned = text.chars().take(2048).collect::<String>();
        if let Ok(Some(v)) =
            tokio::task::spawn_blocking(move || crate::local_embed::embed_one(&owned)).await
        {
            return Ok(v);
        }
        let req = EmbedRequest {
            model: EMBED_MODEL.to_string(),
            input: serde_json::Value::String(text.chars().take(2048).collect()),
        };
        let url = format!("{}/api/embed", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&req)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("ollama embed failed {}: {}", resp.status(), resp.text().await.unwrap_or_default());
        }
        let embed_resp: EmbedResponse = resp.json().await?;
        embed_resp
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("ollama embed: no embeddings in response"))
    }

    /// Embed a batch of texts in one API call.
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        // Local ONNX batch first; fall back to Ollama HTTP on any failure.
        let owned: Vec<String> = texts.iter().map(|t| t.chars().take(2048).collect()).collect();
        if let Ok(Some(v)) = tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
            crate::local_embed::embed_many(&refs)
        })
        .await
        {
            if v.len() == texts.len() {
                return Ok(v);
            }
        }
        let req = EmbedRequest {
            model: EMBED_MODEL.to_string(),
            input: serde_json::Value::Array(
                texts
                    .iter()
                    .map(|t| serde_json::Value::String(t.chars().take(2048).collect()))
                    .collect(),
            ),
        };
        let url = format!("{}/api/embed", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&req)
            .timeout(Duration::from_secs(60))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("ollama embed_batch failed {}", resp.status());
        }
        let embed_resp: EmbedResponse = resp.json().await?;
        Ok(embed_resp.embeddings)
    }

    /// Multimodal generate — describe or reason about one or more images.
    /// Images are read from disk and base64-encoded before sending.
    /// Uses the primary model (llama4:scout natively supports vision).
    /// Requires: `ollama pull llama4:scout`
    pub async fn vision_generate(
        &self,
        prompt: &str,
        image_paths: &[&str],
        system: Option<&str>,
    ) -> Result<GenerateResponse> {
        let mut images = Vec::new();
        for path in image_paths {
            let bytes = std::fs::read(path)
                .map_err(|e| anyhow::anyhow!("vision: could not read image {path}: {e}"))?;
            images.push(base64_encode(&bytes));
        }

        let req = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(str::to_string),
            options: Some(ModelOptions {
                temperature: Some(0.3),
                num_ctx: Some(16384),
                top_p: Some(0.9),
                stop: None,
                think: Some(false),
            }),
            context: None,
            images: Some(images),
            think: Some(false),
        };
        self.generate_with_retry(&req).await
    }

    async fn generate_with_retry(&self, req: &GenerateRequest) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.base_url);
        let mut delay_ms = RETRY_BASE_MS;

        for attempt in 0..MAX_RETRIES {
            match self.http.post(&url).json(req).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(resp.json::<GenerateResponse>().await?);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    if attempt + 1 == MAX_RETRIES {
                        bail!("ollama generate failed {status}: {body}");
                    }
                    warn!("ollama generate attempt {}/{MAX_RETRIES}: {status}", attempt + 1);
                }
                Err(e) => {
                    if attempt + 1 == MAX_RETRIES {
                        bail!("ollama generate connection error: {e}");
                    }
                    warn!("ollama connection attempt {}/{MAX_RETRIES}: {e}", attempt + 1);
                }
            }

            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(16_000);
        }
        unreachable!()
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }

    async fn chat_with_retry(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/api/chat", self.base_url);
        let mut delay_ms = RETRY_BASE_MS;

        for attempt in 0..MAX_RETRIES {
            match self.http.post(&url).json(req).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(resp.json::<ChatResponse>().await?);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    if attempt + 1 == MAX_RETRIES {
                        bail!("ollama chat failed {status}: {body}");
                    }
                    warn!("ollama chat attempt {}/{MAX_RETRIES}: {status}", attempt + 1);
                }
                Err(e) => {
                    if attempt + 1 == MAX_RETRIES {
                        bail!("ollama chat connection error: {e}");
                    }
                    warn!("ollama chat connection attempt {}/{MAX_RETRIES}: {e}", attempt + 1);
                }
            }

            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(16_000);
        }
        unreachable!()
    }
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(TABLE[(b0 >> 2)] as char);
        out.push(TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((b1 & 15) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[b2 & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}
