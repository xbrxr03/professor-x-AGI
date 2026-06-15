use anyhow::Result;
use std::path::Path;

use crate::toolbridge::hashedit::{line_hash, resolve_workspace_path, DEFAULT_HASH_CHARS};

pub const DEFAULT_WINDOW_LINES: usize = 80;
pub const MAX_WINDOW_LINES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWindow {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub output: String,
}

pub fn open_window_file(
    workspace_root: &Path,
    path: &str,
    lines: Option<usize>,
) -> Result<FileWindow> {
    render_window_file(workspace_root, path, 1, lines)
}

pub fn goto_window_file(
    workspace_root: &Path,
    path: &str,
    line: usize,
    lines: Option<usize>,
) -> Result<FileWindow> {
    render_window_file(workspace_root, path, line, lines)
}

pub fn scroll_window_file(
    workspace_root: &Path,
    path: &str,
    start: usize,
    delta: isize,
    lines: Option<usize>,
) -> Result<FileWindow> {
    let next = if delta.is_negative() {
        start.saturating_sub(delta.unsigned_abs())
    } else {
        start.saturating_add(delta as usize)
    }
    .max(1);
    render_window_file(workspace_root, path, next, lines)
}

pub fn render_window_file(
    workspace_root: &Path,
    path: &str,
    start_line: usize,
    lines: Option<usize>,
) -> Result<FileWindow> {
    let resolved = resolve_workspace_path(workspace_root, path);
    let content = std::fs::read_to_string(&resolved)?;
    Ok(render_window_content(path, &content, start_line, lines))
}

pub fn render_window_content(
    path: &str,
    content: &str,
    start_line: usize,
    lines: Option<usize>,
) -> FileWindow {
    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();
    let window_lines = lines
        .unwrap_or(DEFAULT_WINDOW_LINES)
        .clamp(1, MAX_WINDOW_LINES);
    let start_line = start_line.max(1).min(total_lines.max(1));
    let end_line = (start_line + window_lines - 1).min(total_lines);
    let mut output = format!(
        "window {path}: lines {start_line}-{end_line} of {total_lines} (max {window_lines})"
    );

    if total_lines == 0 {
        output.push_str("\n[empty file]");
    } else {
        for (idx, line) in all_lines
            .iter()
            .enumerate()
            .skip(start_line - 1)
            .take(window_lines)
        {
            let line_no = idx + 1;
            output.push('\n');
            output.push_str(&format!(
                "L{}|{}| {}",
                line_no,
                line_hash(line, DEFAULT_HASH_CHARS),
                line
            ));
        }
    }
    if start_line > 1 {
        output.push_str("\n[above: use fs.window_scroll with negative delta or fs.window_goto]");
    }
    if end_line < total_lines {
        output.push_str("\n[below: use fs.window_scroll with positive delta or fs.window_goto]");
    }

    FileWindow {
        path: path.to_string(),
        start_line,
        end_line,
        total_lines,
        output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bounded_line_hash_window() {
        let out = render_window_content("src/lib.rs", "alpha\nbeta\ngamma\n", 2, Some(1));
        assert_eq!(out.start_line, 2);
        assert_eq!(out.end_line, 2);
        assert_eq!(out.total_lines, 3);
        assert!(out.output.contains("window src/lib.rs: lines 2-2 of 3"));
        assert!(out.output.contains("L2|f44| beta"));
        assert!(!out.output.contains("alpha"));
        assert!(!out.output.contains("gamma"));
    }

    #[test]
    fn scroll_uses_saturating_one_based_start() {
        let root = std::env::temp_dir().join(format!("px-window-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "one\ntwo\nthree\nfour\n").unwrap();

        let out = scroll_window_file(&root, "src/lib.rs", 3, -10, Some(2)).unwrap();
        assert_eq!(out.start_line, 1);
        assert!(out.output.contains("L1|"));
        assert!(out.output.contains("L2|"));

        let _ = std::fs::remove_dir_all(root);
    }
}
