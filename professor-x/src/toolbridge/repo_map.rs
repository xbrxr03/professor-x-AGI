//! Repo-map (Aider-style): a ranked, compact map of the codebase's symbols so
//! the agent — and especially the self-evolution loop that edits this very
//! source — navigates the tree by importance instead of blindly grepping.
//!
//! Ranking is PageRank-lite: a symbol's weight is how often its identifier is
//! referenced across the whole tree (a definition everyone calls matters more
//! than a private helper). A file's score is the sum of its defined symbols'
//! reference counts. The map lists the top files with their key definitions.

use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A symbol defined in the tree (function, type, trait, …).
struct Symbol {
    name: String,
    kind: &'static str,
    file: PathBuf,
}

fn def_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Capture the kind and the identifier for common Rust definitions.
        Regex::new(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(fn|struct|enum|trait|type|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)",
        )
        .unwrap()
    })
}

fn ident_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap())
}

/// Recursively collect .rs files under `dir` (skips target/ and hidden dirs).
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Build a ranked repo map rooted at `root`. `focus` (optional) boosts files
/// whose path or symbols contain the keyword. Returns a compact text map of up
/// to `max_files` files.
pub fn build_repo_map(root: &Path, focus: Option<&str>, max_files: usize) -> String {
    // Prefer the crate's src/ (canonical or nested), else the root itself.
    let src = [
        root.join("professor-x/src"),
        root.join("src"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .unwrap_or_else(|| root.to_path_buf());

    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    if files.is_empty() {
        return format!("repo.map: no .rs files found under {}", src.display());
    }

    // Read every file once; collect definitions and a global identifier
    // frequency table (the reference signal).
    let mut contents: HashMap<PathBuf, String> = HashMap::new();
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut global_freq: HashMap<String, u32> = HashMap::new();

    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for cap in def_regex().captures_iter(&text) {
            symbols.push(Symbol {
                kind: match &cap[1] {
                    "fn" => "fn",
                    "struct" => "struct",
                    "enum" => "enum",
                    "trait" => "trait",
                    "type" => "type",
                    "const" => "const",
                    "static" => "static",
                    _ => "item",
                },
                name: cap[2].to_string(),
                file: path.clone(),
            });
        }
        for m in ident_regex().find_iter(&text) {
            *global_freq.entry(m.as_str().to_string()).or_insert(0) += 1;
        }
        contents.insert(path.clone(), text);
    }

    // Group symbols by file and score each file by total inbound references.
    let mut by_file: HashMap<PathBuf, Vec<&Symbol>> = HashMap::new();
    for s in &symbols {
        by_file.entry(s.file.clone()).or_default().push(s);
    }

    let focus_lc = focus.map(|f| f.to_lowercase());
    let mut ranked: Vec<(PathBuf, u64, usize)> = by_file
        .iter()
        .map(|(file, syms)| {
            // file score = sum over defined symbols of (global references - 1 def)
            let mut score: u64 = syms
                .iter()
                .map(|s| global_freq.get(&s.name).copied().unwrap_or(1).saturating_sub(1) as u64)
                .sum();
            if let Some(f) = &focus_lc {
                let path_hit = file.to_string_lossy().to_lowercase().contains(f);
                let sym_hit = syms.iter().any(|s| s.name.to_lowercase().contains(f));
                if path_hit || sym_hit {
                    score = score.saturating_mul(10).saturating_add(1_000_000);
                }
            }
            (file.clone(), score, syms.len())
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));

    let total_files = ranked.len();
    let total_syms = symbols.len();
    let mut out = String::new();
    out.push_str(&format!(
        "repo map — {total_files} files, {total_syms} symbols (ranked by reference frequency{})\n",
        focus.map(|f| format!(", focus='{f}'")).unwrap_or_default()
    ));

    for (file, score, sym_count) in ranked.iter().take(max_files) {
        let rel = file
            .strip_prefix(&src)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        // Show the most-referenced symbols in this file (up to 8).
        let mut syms: Vec<&&Symbol> = by_file.get(file).map(|v| v.iter().collect()).unwrap_or_default();
        syms.sort_by_key(|s| std::cmp::Reverse(global_freq.get(&s.name).copied().unwrap_or(0)));
        let listed: Vec<String> = syms
            .iter()
            .take(8)
            .map(|s| format!("{} {}", s.kind, s.name))
            .collect();
        out.push_str(&format!(
            "\n{rel}  [score {score}, {sym_count} defs]\n  {}\n",
            listed.join(", ")
        ));
    }
    out
}
