use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointManifest {
    pub id: String,
    pub created_at: String,
    pub reason: String,
    pub git_head: Option<String>,
    pub entries: Vec<CheckpointEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointEntry {
    pub path: String,
    pub existed: bool,
    pub blob_oid: Option<String>,
    #[serde(default)]
    pub content: Option<Vec<u8>>,
}

pub fn create_checkpoint(
    workspace_root: &Path,
    paths: &[PathBuf],
    reason: impl Into<String>,
) -> Result<PathBuf> {
    let mut entries = Vec::new();
    for raw in paths {
        let resolved = resolve_inside(workspace_root, raw)?;
        if resolved.is_dir() {
            for file in walk_files(&resolved)? {
                entries.push(capture_one(workspace_root, &file)?);
            }
        } else {
            entries.push(capture_one(workspace_root, &resolved)?);
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.dedup_by(|a, b| a.path == b.path);

    if entries.is_empty() {
        anyhow::bail!("checkpoint requires at least one path");
    }

    let manifest = CheckpointManifest {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        reason: reason.into(),
        git_head: current_head(workspace_root),
        entries,
    };
    let dir = artifact_root(workspace_root)
        .join("checkpoints")
        .join(chrono::Utc::now().format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", manifest.id));
    std::fs::write(&path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(path)
}

pub fn undo_checkpoint(workspace_root: &Path, checkpoint: Option<&str>) -> Result<String> {
    let manifest_path = match checkpoint {
        Some(value) if !value.trim().is_empty() => resolve_checkpoint_ref(workspace_root, value)?,
        _ => latest_checkpoint(workspace_root)?,
    };
    let text = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading checkpoint {}", manifest_path.display()))?;
    let manifest: CheckpointManifest = serde_json::from_str(&text)
        .with_context(|| format!("parsing checkpoint {}", manifest_path.display()))?;

    let mut restored = 0usize;
    let mut removed = 0usize;
    for entry in &manifest.entries {
        let target = resolve_inside(workspace_root, Path::new(&entry.path))?;
        if entry.existed {
            let bytes = if let Some(oid) = entry.blob_oid.as_deref() {
                let output = std::process::Command::new("git")
                    .args(["cat-file", "-p", oid])
                    .current_dir(workspace_root)
                    .output()?;
                if !output.status.success() {
                    anyhow::bail!(
                        "git cat-file {oid} failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                output.stdout
            } else if let Some(content) = &entry.content {
                content.clone()
            } else {
                anyhow::bail!(
                    "checkpoint entry {} missing blob oid and embedded content",
                    entry.path
                );
            };
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, bytes)?;
            restored += 1;
        } else if target.exists() {
            if target.is_dir() {
                std::fs::remove_dir_all(&target)?;
            } else {
                std::fs::remove_file(&target)?;
            }
            removed += 1;
        }
    }

    Ok(format!(
        "undid checkpoint {} from {}: restored {} path(s), removed {} created path(s); manifest={}",
        manifest.id,
        manifest.created_at,
        restored,
        removed,
        manifest_path.display()
    ))
}

fn capture_one(workspace_root: &Path, path: &Path) -> Result<CheckpointEntry> {
    let rel = rel_path(workspace_root, path)?;
    if path.exists() {
        let output = std::process::Command::new("git")
            .arg("hash-object")
            .arg("-w")
            .arg(path)
            .current_dir(workspace_root)
            .output()?;
        if output.status.success() {
            Ok(CheckpointEntry {
                path: rel,
                existed: true,
                blob_oid: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
                content: None,
            })
        } else {
            Ok(CheckpointEntry {
                path: rel,
                existed: true,
                blob_oid: None,
                content: Some(
                    std::fs::read(path)
                        .with_context(|| format!("read checkpoint content {}", path.display()))?,
                ),
            })
        }
    } else {
        Ok(CheckpointEntry {
            path: rel,
            existed: false,
            blob_oid: None,
            content: None,
        })
    }
}

fn latest_checkpoint(workspace_root: &Path) -> Result<PathBuf> {
    let root = artifact_root(workspace_root).join("checkpoints");
    let mut paths = Vec::new();
    if root.exists() {
        collect_json_files(&root, &mut paths)?;
    }
    paths.sort();
    paths
        .pop()
        .ok_or_else(|| anyhow::anyhow!("no checkpoint manifests found under {}", root.display()))
}

fn resolve_checkpoint_ref(workspace_root: &Path, value: &str) -> Result<PathBuf> {
    let value = value.trim();
    let candidate = Path::new(value);
    if candidate.components().count() > 1 || candidate.is_absolute() {
        let resolved = resolve_inside(workspace_root, candidate)?;
        if !resolved.exists() {
            anyhow::bail!("checkpoint path does not exist: {}", resolved.display());
        }
        return Ok(resolved);
    }
    let root = artifact_root(workspace_root).join("checkpoints");
    let mut paths = Vec::new();
    if root.exists() {
        collect_json_files(&root, &mut paths)?;
    }
    let mut matches: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.starts_with(value))
                .unwrap_or(false)
        })
        .collect();
    matches.sort();
    match matches.len() {
        0 => anyhow::bail!("no checkpoint id starts with '{value}'"),
        1 => Ok(matches.remove(0)),
        _ => anyhow::bail!("checkpoint id '{value}' is ambiguous"),
    }
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_files(&path)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}

fn collect_json_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

fn resolve_inside(workspace_root: &Path, path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let normalized = normalize_path(&joined);
    let root = normalize_path(workspace_root);
    if !normalized.starts_with(&root) {
        anyhow::bail!("path escapes workspace: {}", path.display());
    }
    Ok(normalized)
}

fn rel_path(workspace_root: &Path, path: &Path) -> Result<String> {
    let resolved = resolve_inside(workspace_root, path)?;
    let root = normalize_path(workspace_root);
    Ok(resolved
        .strip_prefix(root)?
        .to_string_lossy()
        .trim_start_matches('/')
        .to_string())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn current_head(workspace_root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn artifact_root(workspace_root: &Path) -> PathBuf {
    let nested = workspace_root.join("professor-x/artifacts");
    if nested.exists() {
        nested
    } else {
        workspace_root.join("artifacts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!("px-checkpoint-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn x() {}\n").unwrap();
        let init = std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(init.status.success());
        root
    }

    #[test]
    fn checkpoint_restores_existing_and_removes_created_files() {
        let root = temp_repo();
        let manifest = create_checkpoint(
            &root,
            &[PathBuf::from("src/lib.rs"), PathBuf::from("src/new.rs")],
            "test",
        )
        .unwrap();
        assert!(manifest.exists());
        std::fs::write(root.join("src/lib.rs"), "pub fn x() { 1 }\n").unwrap();
        std::fs::write(root.join("src/new.rs"), "new\n").unwrap();

        let out = undo_checkpoint(&root, None).unwrap();
        assert!(out.contains("restored 1"));
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub fn x() {}\n"
        );
        assert!(!root.join("src/new.rs").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_restores_plain_non_git_workspace() {
        let root =
            std::env::temp_dir().join(format!("px-checkpoint-plain-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/lib.py"),
            "def add(a, b):\n    return a - b\n",
        )
        .unwrap();

        let manifest = create_checkpoint(&root, &[PathBuf::from("src/lib.py")], "plain").unwrap();
        std::fs::write(
            root.join("src/lib.py"),
            "def add(a, b):\n    return a + b\n",
        )
        .unwrap();

        let out = undo_checkpoint(&root, manifest.to_str()).unwrap();
        assert!(out.contains("restored 1"));
        assert_eq!(
            std::fs::read_to_string(root.join("src/lib.py")).unwrap(),
            "def add(a, b):\n    return a - b\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
