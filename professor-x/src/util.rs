//! Small shared helpers for the interactive surfaces (chat REPL, TUI, web).

use std::collections::HashSet;
use std::path::Path;

const MAX_FILE_BYTES: u64 = 60_000;
const MAX_INLINE_CHARS: usize = 8_000;

/// Expand `@path` references in a user task into inline file context — the
/// Claude Code / Cursor pattern. Each `@file` that exists is appended as a fenced
/// block so the agent sees the contents directly. The original `@refs` stay in
/// the text. Missing files / oversized files are skipped silently. Pure string
/// transform, so every entry point (chat, --task, TUI, web) can share it.
pub fn expand_file_refs(input: &str) -> String {
    let mut seen = HashSet::new();
    let mut blocks = String::new();
    for tok in input.split_whitespace() {
        let Some(raw) = tok.strip_prefix('@') else {
            continue;
        };
        // trim trailing punctuation a user might type after the path
        let path = raw.trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | ']'));
        if path.is_empty() || !seen.insert(path.to_string()) {
            continue;
        }
        let p = Path::new(path);
        if !p.is_file() {
            continue;
        }
        let too_big = p
            .metadata()
            .map(|m| m.len() > MAX_FILE_BYTES)
            .unwrap_or(true);
        if too_big {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(p) {
            let snippet: String = content.chars().take(MAX_INLINE_CHARS).collect();
            let truncated = content.chars().count() > MAX_INLINE_CHARS;
            blocks.push_str(&format!(
                "\n\nContents of {path}:\n```\n{snippet}{}\n```",
                if truncated { "\n[... truncated]" } else { "" }
            ));
        }
    }
    if blocks.is_empty() {
        input.to_string()
    } else {
        format!("{input}{blocks}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_refs_passthrough() {
        assert_eq!(expand_file_refs("just a task"), "just a task");
    }

    #[test]
    fn missing_file_skipped() {
        let out = expand_file_refs("look at @/no/such/file.rs please");
        assert_eq!(out, "look at @/no/such/file.rs please");
    }

    #[test]
    fn existing_file_inlined() {
        let dir = std::env::temp_dir().join("px_util_test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("hello.txt");
        std::fs::write(&f, "hi there").unwrap();
        let task = format!("summarize @{}", f.display());
        let out = expand_file_refs(&task);
        assert!(out.contains("Contents of"));
        assert!(out.contains("hi there"));
    }
}
