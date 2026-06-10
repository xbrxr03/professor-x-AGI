use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::toolbridge::hashedit::resolve_workspace_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPatchFile {
    pub path: String,
    pub resolved_path: PathBuf,
    pub before: String,
    pub after: String,
    pub fuzzy_matches: usize,
}

#[derive(Debug, Clone)]
struct FilePatch {
    path: String,
    hunks: Vec<Hunk>,
}

#[derive(Debug, Clone)]
struct Hunk {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

pub fn apply_fuzzy_patch_to_memory(
    workspace_root: &Path,
    patch: &str,
) -> Result<Vec<AppliedPatchFile>> {
    let files = parse_unified_patch(patch)?;
    let mut applied = Vec::new();
    for file_patch in files {
        let resolved_path = resolve_workspace_path(workspace_root, &file_patch.path);
        let before = match std::fs::read_to_string(&resolved_path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err).with_context(|| format!("read {}", resolved_path.display())),
        };
        let trailing_newline = before.ends_with('\n') || before.is_empty();
        let (after, fuzzy_matches) =
            apply_hunks_to_content(&before, trailing_newline, &file_patch.hunks)?;
        if before != after {
            applied.push(AppliedPatchFile {
                path: file_patch.path,
                resolved_path,
                before,
                after,
                fuzzy_matches,
            });
        }
    }
    if applied.is_empty() {
        bail!("fuzzy patch produced no file changes");
    }
    Ok(applied)
}

fn parse_unified_patch(patch: &str) -> Result<Vec<FilePatch>> {
    let mut files = Vec::<FilePatch>::new();
    let mut current_path: Option<String> = None;
    let mut current_hunks = Vec::<Hunk>::new();
    let mut current_hunk: Option<Hunk> = None;

    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush_hunk(&mut current_hunks, &mut current_hunk);
            flush_file(&mut files, &mut current_path, &mut current_hunks)?;
            let mut parts = rest.split_whitespace();
            let _old = parts.next();
            current_path = parts
                .next()
                .and_then(clean_patch_path)
                .filter(|path| path != "/dev/null");
            continue;
        }
        if let Some(raw) = line.strip_prefix("+++ ") {
            if let Some(path) = clean_patch_path(raw) {
                if path != "/dev/null" {
                    current_path = Some(path);
                }
            }
            continue;
        }
        if line.starts_with("@@") {
            flush_hunk(&mut current_hunks, &mut current_hunk);
            current_hunk = Some(Hunk {
                old_lines: Vec::new(),
                new_lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };
        if line == "\\ No newline at end of file" {
            continue;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            hunk.old_lines.push(rest.to_string());
            hunk.new_lines.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix('-') {
            hunk.old_lines.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix('+') {
            hunk.new_lines.push(rest.to_string());
        } else {
            bail!("unsupported unified diff hunk line: {line}");
        }
    }

    flush_hunk(&mut current_hunks, &mut current_hunk);
    flush_file(&mut files, &mut current_path, &mut current_hunks)?;
    if files.is_empty() {
        bail!("patch contains no unified diff hunks");
    }
    Ok(files)
}

fn flush_hunk(hunks: &mut Vec<Hunk>, current_hunk: &mut Option<Hunk>) {
    if let Some(hunk) = current_hunk.take() {
        hunks.push(hunk);
    }
}

fn flush_file(
    files: &mut Vec<FilePatch>,
    current_path: &mut Option<String>,
    current_hunks: &mut Vec<Hunk>,
) -> Result<()> {
    if current_hunks.is_empty() {
        return Ok(());
    }
    let Some(path) = current_path.take() else {
        bail!("patch hunk has no target file");
    };
    files.push(FilePatch {
        path,
        hunks: std::mem::take(current_hunks),
    });
    Ok(())
}

fn clean_patch_path(raw: &str) -> Option<String> {
    let path = raw.split_whitespace().next()?;
    if path == "/dev/null" {
        return Some(path.to_string());
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .map(ToString::to_string)
}

fn apply_hunks_to_content(
    content: &str,
    trailing_newline: bool,
    hunks: &[Hunk],
) -> Result<(String, usize)> {
    let mut lines: Vec<String> = if content.is_empty() {
        Vec::new()
    } else {
        content.lines().map(ToString::to_string).collect()
    };
    let mut fuzzy_matches = 0usize;

    for hunk in hunks {
        let (index, fuzzy) = find_hunk_index(&lines, &hunk.old_lines)?;
        if fuzzy {
            fuzzy_matches += 1;
        }
        lines.splice(index..index + hunk.old_lines.len(), hunk.new_lines.clone());
    }

    let mut out = lines.join("\n");
    if trailing_newline && !out.is_empty() {
        out.push('\n');
    }
    Ok((out, fuzzy_matches))
}

fn find_hunk_index(lines: &[String], old_lines: &[String]) -> Result<(usize, bool)> {
    if old_lines.is_empty() {
        return Ok((0, false));
    }
    if let Some(index) = find_exact(lines, old_lines) {
        return Ok((index, false));
    }
    let matches = find_normalized(lines, old_lines);
    match matches.as_slice() {
        [index] => Ok((*index, true)),
        [] => bail!("patch hunk context not found exactly or by normalized whitespace"),
        _ => bail!("patch hunk context is ambiguous under normalized whitespace"),
    }
}

fn find_exact(lines: &[String], needle: &[String]) -> Option<usize> {
    lines
        .windows(needle.len())
        .position(|window| window.iter().zip(needle).all(|(a, b)| a == b))
}

fn find_normalized(lines: &[String], needle: &[String]) -> Vec<usize> {
    let needle_norm: Vec<String> = needle.iter().map(|line| normalize_line(line)).collect();
    lines
        .windows(needle.len())
        .enumerate()
        .filter_map(|(idx, window)| {
            let matches = window
                .iter()
                .map(|line| normalize_line(line))
                .zip(&needle_norm)
                .all(|(a, b)| &a == b);
            matches.then_some(idx)
        })
        .collect()
}

fn normalize_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_patch_applies_under_whitespace_drift() {
        let root = std::env::temp_dir().join(format!("px-fuzzy-patch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn add(left: i32, right: i32) -> i32 {\n        left - right\n}\n",
        )
        .unwrap();
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn add(left: i32, right: i32) -> i32 {\n-    left - right\n+    left + right\n }\n";

        let applied = apply_fuzzy_patch_to_memory(&root, patch).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].fuzzy_matches, 1);
        assert!(applied[0].after.contains("    left + right"));
        assert!(std::fs::read_to_string(root.join("src/lib.rs"))
            .unwrap()
            .contains("        left - right"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ambiguous_normalized_context_is_rejected() {
        let root = std::env::temp_dir().join(format!("px-fuzzy-patch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "same line\nsame   line\n").unwrap();
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-same    line\n+changed\n";

        let err = apply_fuzzy_patch_to_memory(&root, patch).unwrap_err();
        assert!(err.to_string().contains("ambiguous"));

        let _ = std::fs::remove_dir_all(root);
    }
}
