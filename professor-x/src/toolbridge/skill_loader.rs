/// SKILL.md parser — 3-tier progressive disclosure.
/// Spec from K-Dense-AI/scientific-agent-skills.
///
/// Tier 1 (startup): name + description from frontmatter only (~100 tokens per skill)
/// Tier 2 (activation): full SKILL.md body loaded when LLM selects the skill
/// Tier 3 (on demand): scripts/, references/, assets/ loaded when referenced
use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::warn;

const EPHEMERAL_PROVENANCE_SKILL_PREFIXES: &[&str] = &[
    "px-operator-goal-",
    "px-operator-autocommit-",
    "px-autonomous-patch-",
];

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
    if !name
        .chars()
        .next()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false)
    {
        bail!("skill name must start with [a-z0-9]: '{name}'");
    }
    if !name
        .chars()
        .last()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false)
    {
        bail!("skill name must end with [a-z0-9]: '{name}'");
    }
    if name.contains("--") {
        bail!("skill name cannot contain consecutive hyphens: '{name}'");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
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
    let end = content[3..]
        .find("\n---")
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
    if let Some(dir_name) = skill_md_path
        .parent()
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

/// One structured verification check for a skill (mirrors the artifact-truth layer).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// The verdict that a skill is a usable first-class runtime unit (item #4).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillVerification {
    pub skill: String,
    pub path: String,
    pub passed: bool,
    pub checks: Vec<SkillCheck>,
}

/// Verify a skill is a well-formed runtime unit before it is trusted at runtime: a valid name, a
/// real description, a non-trivial body (an actual procedure, not a stub), and a coherent
/// permission scope (declared `allowed-tools`, if present, must be non-empty and well-formed).
/// Pure over (frontmatter, content) so it is unit-testable.
pub fn verify_skill(fm: &SkillFrontmatter, full_content: &str, path: &Path) -> SkillVerification {
    let chk = |name: &str, passed: bool, detail: String| SkillCheck {
        name: name.to_string(),
        passed,
        detail,
    };
    let mut checks = Vec::new();

    checks.push(match validate_skill_name(&fm.name) {
        Ok(()) => chk("name", true, fm.name.clone()),
        Err(e) => chk("name", false, e.to_string()),
    });

    let desc_len = fm.description.trim().len();
    checks.push(chk(
        "description",
        desc_len >= 12,
        if desc_len >= 12 {
            format!("{desc_len} chars")
        } else {
            "description missing or too short (need >=12 chars)".to_string()
        },
    ));

    let body_len = skill_body(full_content).trim().len();
    checks.push(chk(
        "body",
        body_len >= 40,
        if body_len >= 40 {
            format!("{body_len} chars of procedure")
        } else {
            "body missing or too short (need >=40 chars) — not a usable runtime unit".to_string()
        },
    ));

    let scope = match &fm.allowed_tools {
        None => chk(
            "permission_scope",
            true,
            "no allowed-tools declared (inherits default scope)".to_string(),
        ),
        Some(tools) if tools.is_empty() => chk(
            "permission_scope",
            false,
            "allowed-tools declared but empty".to_string(),
        ),
        Some(tools) if tools.iter().any(|t| t.trim().is_empty()) => chk(
            "permission_scope",
            false,
            "allowed-tools contains empty entries".to_string(),
        ),
        Some(tools) => chk(
            "permission_scope",
            true,
            format!("{} tool(s) granted", tools.len()),
        ),
    };
    checks.push(scope);

    let passed = checks.iter().all(|c| c.passed);
    SkillVerification {
        skill: fm.name.clone(),
        path: path.display().to_string(),
        passed,
        checks,
    }
}

/// Body = the content after the closing frontmatter `---`.
fn skill_body(content: &str) -> &str {
    let trimmed = content.trim_start();
    if trimmed.starts_with("---") {
        if let Some(end) = trimmed[3..].find("\n---") {
            return &trimmed[3 + end + 4..];
        }
    }
    content
}

/// Scan a skills directory and return all valid Tier 1 frontmatters.
pub fn scan_skills_dir(skills_dir: &Path) -> Vec<(SkillFrontmatter, std::path::PathBuf)> {
    let mut results = Vec::new();
    scan_skills_dir_inner(skills_dir, &mut results);
    results.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    results
}

fn is_ephemeral_provenance_skill_name(name: &str) -> bool {
    EPHEMERAL_PROVENANCE_SKILL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn scan_skills_dir_inner(skills_dir: &Path, results: &mut Vec<(SkillFrontmatter, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(skills_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                match load_tier1(&skill_md) {
                    Ok(fm) => {
                        if is_ephemeral_provenance_skill_name(&fm.name) {
                            warn!("skipping ephemeral provenance skill {:?}", skill_md);
                            continue;
                        }
                        results.push((fm, skill_md));
                    }
                    Err(e) => warn!("skipping {:?}: {e}", path),
                }
            } else {
                scan_skills_dir_inner(&path, results);
            }
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            match load_legacy_markdown_skill(&path) {
                Ok(fm) => {
                    if is_ephemeral_provenance_skill_name(&fm.name) {
                        warn!("skipping ephemeral provenance skill {:?}", path);
                        continue;
                    }
                    results.push((fm, path));
                }
                Err(e) => warn!("skipping {:?}: {e}", path),
            }
        }
    }
}

fn load_legacy_markdown_skill(path: &Path) -> Result<SkillFrontmatter> {
    let content = std::fs::read_to_string(path)?;
    if content.trim_start().starts_with("---") {
        return parse_frontmatter(&content);
    }

    let name = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow::anyhow!("cannot infer skill name"))?;
    validate_skill_name(&name)?;

    let description = extract_purpose(&content)
        .or_else(|| extract_non_status_line(&content))
        .unwrap_or_else(|| "Legacy project skill; see body for details.".to_string());

    Ok(SkillFrontmatter {
        name,
        description,
        license: None,
        compatibility: Some("legacy-markdown".to_string()),
        allowed_tools: None,
        metadata: None,
    })
}

fn extract_purpose(content: &str) -> Option<String> {
    let mut in_purpose = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            in_purpose = trimmed.eq_ignore_ascii_case("## Purpose");
            continue;
        }
        if in_purpose && !trimmed.is_empty() && !trimmed.starts_with('#') {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_non_status_line(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("```")
                && !line.to_ascii_lowercase().contains("status: stub")
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_skill_accepts_a_well_formed_runtime_unit() {
        let content = "---\nname: px-good-skill\ndescription: Recover cleanly from a failed tool call\nallowed-tools:\n  - fs.read\n  - shell.restricted\n---\n\nWhen a tool fails: inspect the observation, validate outputs, choose one bounded retry, recover.\n";
        let fm = parse_frontmatter(content).unwrap();
        let v = verify_skill(&fm, content, Path::new("px-good-skill/SKILL.md"));
        assert!(v.passed, "well-formed skill should verify: {:?}", v.checks);
    }

    #[test]
    fn verify_skill_rejects_stub_body_and_empty_scope() {
        // Real frontmatter but an empty body and an empty allowed-tools list = not a runtime unit.
        let content = "---\nname: px-stub\ndescription: A reasonable description here\nallowed-tools: []\n---\n";
        let fm = parse_frontmatter(content).unwrap();
        let v = verify_skill(&fm, content, Path::new("px-stub/SKILL.md"));
        assert!(!v.passed);
        assert!(v.checks.iter().any(|c| c.name == "body" && !c.passed));
        assert!(v.checks.iter().any(|c| c.name == "permission_scope" && !c.passed));
    }

    #[test]
    fn legacy_markdown_skill_loads_from_heading_and_purpose() {
        let dir = std::env::temp_dir().join(format!("px-skill-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("conductor")).unwrap();
        let path = dir.join("conductor/px-daily-cycle.md");
        std::fs::write(
            &path,
            "# px-daily-cycle\n\n## Purpose\nRun the full autonomous research day.\n",
        )
        .unwrap();

        let skills = scan_skills_dir(&dir);

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0.name, "px-daily-cycle");
        assert_eq!(
            skills[0].0.description,
            "Run the full autonomous research day."
        );
    }

    #[test]
    fn scan_skills_dir_skips_ephemeral_operator_provenance_skills() {
        let dir = std::env::temp_dir().join(format!("px-skill-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("conductor")).unwrap();
        std::fs::write(
            dir.join("conductor/px-operator-goal-20260616-visible-work.md"),
            "# px-operator-goal-20260616-visible-work\n\nOperator goal: make work visible.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("conductor/retry-plan-generation.md"),
            "# retry-plan-generation\n\n## Purpose\nRecover after a failed first tool choice.\n",
        )
        .unwrap();

        let skills = scan_skills_dir(&dir);

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0.name, "retry-plan-generation");
    }
}
