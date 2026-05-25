/// SKILL.md parser — 3-tier progressive disclosure.
/// Spec from K-Dense-AI/scientific-agent-skills.
///
/// Tier 1 (startup): name + description from frontmatter only (~100 tokens per skill)
/// Tier 2 (activation): full SKILL.md body loaded when LLM selects the skill
/// Tier 3 (on demand): scripts/, references/, assets/ loaded when referenced

use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::Path;
use tracing::warn;

/// Parsed SKILL.md frontmatter (Tier 1 fields only).
#[derive(Debug, Clone, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<Vec<String>>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Name validation from K-Dense-AI SKILL.md spec.
/// Pattern: ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ — max 64 chars, no consecutive hyphens.
pub fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        bail!("skill name must be 1–64 characters: '{name}'");
    }
    if !name.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
        bail!("skill name must start with [a-z0-9]: '{name}'");
    }
    if !name.chars().last().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
        bail!("skill name must end with [a-z0-9]: '{name}'");
    }
    if name.contains("--") {
        bail!("skill name cannot contain consecutive hyphens: '{name}'");
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        bail!("skill name may only contain [a-z0-9-]: '{name}'");
    }
    Ok(())
}

/// Parse SKILL.md frontmatter from file content (Tier 1).
pub fn parse_frontmatter(content: &str) -> Result<SkillFrontmatter> {
    let content = content.trim();
    if !content.starts_with("---") {
        bail!("SKILL.md missing YAML frontmatter (expected '---' at start)");
    }
    let end = content[3..].find("\n---")
        .ok_or_else(|| anyhow::anyhow!("SKILL.md frontmatter not closed with '---'"))?;
    let yaml = &content[3..end + 3];
    let fm: SkillFrontmatter = serde_yaml::from_str(yaml)?;
    validate_skill_name(&fm.name)?;
    Ok(fm)
}

/// Load Tier 1 frontmatter from a SKILL.md file path.
pub fn load_tier1(skill_md_path: &Path) -> Result<SkillFrontmatter> {
    let content = std::fs::read_to_string(skill_md_path)?;
    let fm = parse_frontmatter(&content)?;

    // Warn if skill directory name doesn't match declared name field.
    if let Some(dir_name) = skill_md_path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        if dir_name != fm.name {
            warn!(
                "SKILL.md name mismatch: directory '{}' vs declared name '{}'",
                dir_name, fm.name
            );
        }
    }

    Ok(fm)
}

/// Load full SKILL.md body (Tier 2) — called when LLM selects a skill.
pub fn load_tier2(skill_md_path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(skill_md_path)?)
}

/// Scan a skills directory and return all valid Tier 1 frontmatters.
pub fn scan_skills_dir(skills_dir: &Path) -> Vec<(SkillFrontmatter, std::path::PathBuf)> {
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(skills_dir) else { return results };

    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.exists() {
            match load_tier1(&skill_md) {
                Ok(fm) => results.push((fm, skill_md)),
                Err(e) => warn!("skipping {:?}: {e}", entry.path()),
            }
        }
    }
    results
}
