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
    // Weak local models frequently INVENT the line hash (e.g. "abc", "e3e") even when their
    // edit is correct — a strict hash check then blocks correct edits while providing no real
    // safety (a fabricated hash guarantees nothing). Try strict first; on a hash mismatch,
    // fall back to a line-based apply. The caller runs editverify on the candidate, so a
    // wrong-line edit that breaks the file is still rejected — that is the real guard.
    let (updated, note) = match hash_edit_content(&original, line, expected_hash, new_text) {
        Ok(u) => (u, ""),
        Err(e) if e.to_string().contains("stale line hash") => (
            apply_by_line(&original, line, new_text)?,
            " (hash mismatch; applied by line, verified by lint)",
        ),
        Err(e) => return Err(e),
    };
    if mode == "apply" {
        std::fs::write(&resolved, &updated)?;
    }

    Ok(HashEditOutcome {
        summary: format!("hash_edit {mode} {path} line {line}{note}"),
        before: original,
        after: updated,
    })
}

/// Apply `new_text` to a 1-based line without a hash check (forgiving fallback for weak
/// models that invent hashes). Correctness is guarded downstream by editverify.
fn apply_by_line(content: &str, line: usize, new_text: &str) -> Result<String> {
    let mut lines = split_preserving_trailing_newline(content);
    let idx = line - 1;
    let Some(current) = lines.get_mut(idx) else {
        bail!(
            "line {line} is outside file; file has {} line(s)",
            lines.len()
        );
    };
    reject_obvious_python_wrong_line_edit(current, new_text)?;
    *current = new_text.to_string();
    Ok(join_preserving_final_newline(
        &lines,
        content.ends_with('\n'),
    ))
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

    reject_obvious_python_wrong_line_edit(current, new_text)?;
    *current = new_text.to_string();
    Ok(join_preserving_final_newline(
        &lines,
        content.ends_with('\n'),
    ))
}

fn reject_obvious_python_wrong_line_edit(current: &str, new_text: &str) -> Result<()> {
    let current_trimmed = current.trim_start();
    let new_trimmed = new_text.trim_start();
    let new_is_indented = new_text.starts_with(' ') || new_text.starts_with('\t');
    if current_trimmed.starts_with("def ")
        && current_trimmed.ends_with(':')
        && new_is_indented
        && (new_trimmed.starts_with("return ")
            || new_trimmed.starts_with("raise ")
            || new_trimmed.starts_with("if ")
            || new_trimmed.starts_with("for ")
            || new_trimmed.starts_with("while "))
    {
        bail!(
            "refusing likely wrong-line Python edit: attempted to replace function definition `{}` with indented body `{}`; edit the function body line instead",
            current_trimmed,
            new_trimmed
        );
    }
    Ok(())
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

    #[test]
    fn hash_edit_rejects_python_def_replaced_by_indented_body() {
        let hash = line_hash("def mul(a, b):", 3);
        let err = hash_edit_content(
            "def mul(a, b):\n    return a + b\n",
            1,
            &hash,
            "    return a * b",
        )
        .unwrap_err();
        assert!(err.to_string().contains("wrong-line Python edit"));
    }

    #[test]
    fn hash_edit_line_fallback_rejects_python_def_replaced_by_indented_body() {
        let err =
            apply_by_line("def mul(a, b):\n    return a + b\n", 1, "    return a * b").unwrap_err();
        assert!(err.to_string().contains("wrong-line Python edit"));
    }
}
