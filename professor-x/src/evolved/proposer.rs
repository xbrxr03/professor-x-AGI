/// Evolution proposer — Researcher role from ASI-Evolve.
/// Generates EvolutionNode proposals with AHE ChangeManifest contracts.
///
/// Node schema: ASI-Evolve utils/structures.py Node dataclass
/// Diff format: ASI-Evolve config.yaml diff_pattern (<<<< SEARCH / ==== / >>>> REPLACE)
/// UCB1 sampling: ASI-Evolve config.yaml ucb1_c = 1.414
/// ChangeManifest: AHE paper arXiv:2604.25850, Section 3.3 Decision Observability
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HarnessComponent {
    SystemPrompt,
    ToolDescription(String),
    SkillDefinition(String),
    HarnessConfig,
    ProceduralMemory,
    /// Human approval required — never autonomous.
    Middleware,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Proposed,
    Testing,
    Accepted,
    Rejected,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationStatus {
    Pending,
    Confirmed,
    Rejected,
}

/// AHE change manifest — required for every evolution proposal.
/// Source: AHE paper (arXiv:2604.25850), Section 3.3 Decision Observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeManifest {
    pub evidence_cited: Vec<String>,
    pub root_cause: String,
    pub fix_description: String,
    pub predicted_fixes: Vec<String>,
    pub predicted_regressions: Vec<String>,
    pub verification_status: VerificationStatus,
    pub verified_at: Option<DateTime<Utc>>,
}

/// One evolution candidate. Mirrors ASI-Evolve Node dataclass (utils/structures.py).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionNode {
    /// Sequential int ID assigned by DB (ASI-Evolve uses int IDs).
    pub id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub parent_ids: Vec<i64>,
    pub motivation: String,
    pub target_component: HarnessComponent,
    /// ASI-Evolve diff format: <<<<<<< SEARCH\n...\n=======\n...\n>>>>>>> REPLACE
    pub diff: String,
    pub results: serde_json::Value,
    pub analysis: String,
    pub manifest: ChangeManifest,
    pub score: f32,
    pub visit_count: u32,
    pub status: NodeStatus,
}

impl EvolutionNode {
    pub fn new(
        motivation: String,
        target: HarnessComponent,
        diff: String,
        manifest: ChangeManifest,
    ) -> Self {
        Self {
            id: None,
            created_at: Utc::now(),
            parent_ids: Vec::new(),
            motivation,
            target_component: target,
            diff,
            results: serde_json::Value::Null,
            analysis: String::new(),
            manifest,
            score: 0.0,
            visit_count: 0,
            status: NodeStatus::Proposed,
        }
    }
}

pub struct NodeDatabase {
    db: Arc<Mutex<Connection>>,
}

impl NodeDatabase {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn insert(&self, node: &mut EvolutionNode) -> Result<i64> {
        let db = self.db.lock().unwrap();
        let parent_ids_json = serde_json::to_string(&node.parent_ids)?;
        let component_str = format!("{:?}", node.target_component);
        let manifest_json = serde_json::to_string(&node.manifest)?;
        db.execute(
            "INSERT INTO evolution_nodes
             (created_at, parent_ids, motivation, target_component, diff, results,
              analysis, manifest, score, visit_count, status)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                node.created_at.to_rfc3339(),
                parent_ids_json,
                node.motivation,
                component_str,
                node.diff,
                node.results.to_string(),
                node.analysis,
                manifest_json,
                node.score,
                node.visit_count,
                format!("{:?}", node.status),
            ],
        )?;
        let id = db.last_insert_rowid();
        node.id = Some(id);
        Ok(id)
    }

    /// UCB1 sampling — c=1.414 from ASI-Evolve config.yaml.
    /// score(node) = normalized_score + 1.414 * sqrt(ln(N_total) / visit_count)
    /// Unvisited nodes → infinite priority (sampled first).
    pub fn sample_ucb1(&self, n: usize) -> Result<Vec<EvolutionNode>> {
        let db = self.db.lock().unwrap();
        let total: i64 = db.query_row(
            "SELECT COUNT(*) FROM evolution_nodes WHERE status IN ('Proposed', 'Accepted')",
            [],
            |r| r.get(0),
        )?;

        if total == 0 {
            return Ok(Vec::new());
        }

        let n_total = total as f64;
        let c = 1.414_f64;

        let mut stmt = db.prepare(
            "SELECT id, created_at, parent_ids, motivation, target_component, diff,
                    results, analysis, manifest, score, visit_count, status
             FROM evolution_nodes
             WHERE status IN ('Proposed', 'Accepted')
             ORDER BY id DESC LIMIT 50",
        )?;

        struct Row {
            ucb: f64,
            id: i64,
            created_at: String,
            parent_ids: String,
            motivation: String,
            target_component: String,
            diff: String,
            results: String,
            analysis: String,
            manifest: String,
            score: f64,
            visit_count: i64,
            status: String,
        }

        let mut rows: Vec<Row> = stmt
            .query_map([], |row| {
                let visit_count: i64 = row.get(10)?;
                let score: f64 = row.get(9)?;
                let ucb = if visit_count == 0 {
                    f64::MAX
                } else {
                    score + c * (n_total.ln() / visit_count as f64).sqrt()
                };
                Ok(Row {
                    ucb,
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    parent_ids: row.get(2)?,
                    motivation: row.get(3)?,
                    target_component: row.get(4)?,
                    diff: row.get(5)?,
                    results: row.get(6)?,
                    analysis: row.get(7)?,
                    manifest: row.get(8)?,
                    score,
                    visit_count,
                    status: row.get(11)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        rows.sort_by(|a, b| {
            b.ucb
                .partial_cmp(&a.ucb)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(rows
            .into_iter()
            .take(n)
            .map(|r| {
                let default_manifest = ChangeManifest {
                    evidence_cited: Vec::new(),
                    root_cause: String::new(),
                    fix_description: String::new(),
                    predicted_fixes: Vec::new(),
                    predicted_regressions: Vec::new(),
                    verification_status: VerificationStatus::Pending,
                    verified_at: None,
                };
                EvolutionNode {
                    id: Some(r.id),
                    created_at: DateTime::parse_from_rfc3339(&r.created_at)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    parent_ids: serde_json::from_str(&r.parent_ids).unwrap_or_default(),
                    motivation: r.motivation,
                    target_component: HarnessComponent::HarnessConfig,
                    diff: r.diff,
                    results: serde_json::from_str(&r.results).unwrap_or(serde_json::Value::Null),
                    analysis: r.analysis,
                    manifest: serde_json::from_str(&r.manifest).unwrap_or(default_manifest),
                    score: r.score as f32,
                    visit_count: r.visit_count as u32,
                    status: match r.status.as_str() {
                        "Testing" => NodeStatus::Testing,
                        "Accepted" => NodeStatus::Accepted,
                        "Rejected" => NodeStatus::Rejected,
                        "RolledBack" => NodeStatus::RolledBack,
                        _ => NodeStatus::Proposed,
                    },
                }
            })
            .collect())
    }
}
