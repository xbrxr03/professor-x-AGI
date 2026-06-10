use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const DEFAULT_HASH_CHARS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashEditOutcome {
    pub summary: String,
    pub before: String,
    pub after: String,
}

pub fn resolve_workspace_path(workspace_root: &Path, path: &str) -> PathBuf {
    let path_ref = Path::new(path);
    if path_ref.is_absolute() {
        path_ref.to_path_buf()
    } else {
        workspace_root.join(path_ref)
    }
}

pub fn hash_read_file(workspace_root: &Path, path: &str) -> Result<String> {
    let resolved = resolve_workspace_path(workspace_root, path);
    let content = std::fs::read_to_string(&resolved)?;
    Ok(hash_read_content(&content, DEFAULT_HASH_CHARS))
}

pub fn hash_read_content(content: &str, hash_chars: usize) -> String {
    content
        .lines()
        .enumerate()
        .map(|(idx, line)| format!("L{}|{}| {}", idx + 1, line_hash(line, hash_chars), line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn hash_edit_file(
    workspace_root: &Path,
    path: &str,
    line: usize,
    expected_hash: &str,
    new_text: &str,
    mode: &str,
) -> Result<HashEditOutcome> {
    if line == 0 {
        bail!("fs.hash_edit line must be 1-based");
    }
    if expected_hash.trim().is_empty() {
        bail!("fs.hash_edit requires non-empty 'hash'");
    }
    if !matches!(mode, "check" | "apply") {
        bail!("fs.hash_edit mode must be 'check' or 'apply'");
    }

    let resolved = resolve_workspace_path(workspace_root, path);
    let original = std::fs::read_to_string(&resolved)?;
    let updated = hash_edit_content(&original, line, expected_hash, new_text)?;
    if mode == "apply" {
        std::fs::write(&resolved, &updated)?;
    }

    Ok(HashEditOutcome {
        summary: format!("hash_edit {mode} {path} line {line}"),
        before: original,
        after: updated,
    })
}

pub fn hash_edit_content(
    content: &str,
    line: usize,
    expected_hash: &str,
    new_text: &str,
) -> Result<String> {
    if line == 0 {
        bail!("line must be 1-based");
    }
    let mut lines = split_preserving_trailing_newline(content);
    let idx = line - 1;
    let Some(current) = lines.get_mut(idx) else {
        bail!(
            "line {line} is outside file; file has {} line(s)",
            lines.len()
        );
    };

    let actual_hash = line_hash(
        current,
        expected_hash.chars().count().max(DEFAULT_HASH_CHARS),
    );
    if actual_hash != expected_hash {
        bail!(
            "stale line hash at L{line}: expected {expected_hash}, current {actual_hash}; re-read with fs.hash_read before editing"
        );
    }

    *current = new_text.to_string();
    Ok(join_preserving_final_newline(
        &lines,
        content.ends_with('\n'),
    ))
}

pub fn line_hash(line: &str, chars: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(line.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    hex.chars().take(chars.max(1)).collect()
}

fn split_preserving_trailing_newline(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }
    content.lines().map(ToString::to_string).collect()
}

fn join_preserving_final_newline(lines: &[String], trailing_newline: bool) -> String {
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_read_labels_lines_with_stable_short_hashes() {
        let rendered = hash_read_content("alpha\nbeta\n", 3);
        assert!(rendered.contains("L1|8ed| alpha"));
        assert!(rendered.contains("L2|f44| beta"));
    }

    #[test]
    fn hash_edit_updates_matching_line() {
        let hash = line_hash("beta", 3);
        let edited = hash_edit_content("alpha\nbeta\n", 2, &hash, "gamma").unwrap();
        assert_eq!(edited, "alpha\ngamma\n");
    }

    #[test]
    fn hash_edit_rejects_stale_hash_without_change() {
        let err = hash_edit_content("alpha\nbeta\n", 2, "bad", "gamma").unwrap_err();
        assert!(err.to_string().contains("stale line hash"));
    }
}
