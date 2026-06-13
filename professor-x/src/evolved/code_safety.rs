//! Safety primitive for the autonomous M4 code-proposer (no human in the loop, so the safety
//! must be STRUCTURAL — see docs/research/m4-code-proposer-scoping.md).
//!
//! A self-editing, benchmark-rewarded loop has a strong incentive to game the metric instead of
//! improving the agent (edit the fixtures, weaken the runner/lint, hardcode answers, relax the
//! eval). The defense is **default-deny**: a proposed diff may ONLY touch the files of the
//! diagnosed component, and may NEVER touch the benchmark, the evaluator, the safety gates, the
//! tests, or identity — even if the component list somehow named them.

use std::collections::HashSet;

/// Path fragments that a proposed code diff may NEVER touch, regardless of the allow-list.
/// These are the surfaces a metric-gaming or misevolving change would target.
pub const HARD_FORBIDDEN: &[&str] = &[
    "scripts/benchmarks/",         // the repo-fix fixtures + check.py (the reward signal)
    "src/evolved/hiro.rs",         // the HIRO evaluator / judge
    "src/evolved/code_safety.rs",  // this guard itself
    "src/policyd/",                // security gating, audit, permissions
    "src/memd/",                   // memory internals / identity store
    "config/",                     // model/hardware config
    "professor_x.md",              // identity seed
    "Cargo.toml",                  // build/deps
    ".github/",
];

/// Extra signals inside a diff body that indicate eval/test tampering even on an allowed file.
pub const FORBIDDEN_BODY_SIGNALS: &[&str] = &[
    "repo_fix_measure",            // the benchmark runner
    "run_repo_fix_bench",
    "expect_exit",                 // fixture pass/fail contract
    "#[cfg(test)]",                // adding/altering tests to pass
    "fn analyze_reward_hacking",   // the reward-hacking scanner
];

/// Parse the file paths a unified diff touches (from `+++ b/<path>` / `--- a/<path>` lines).
pub fn diff_paths(diff: &str) -> HashSet<String> {
    let mut paths = HashSet::new();
    for line in diff.lines() {
        for prefix in ["+++ ", "--- "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let p = rest.trim();
                if p == "/dev/null" {
                    continue;
                }
                // strip the a/ or b/ git prefix and any trailing tab metadata
                let p = p.split('\t').next().unwrap_or(p);
                let p = p
                    .strip_prefix("a/")
                    .or_else(|| p.strip_prefix("b/"))
                    .unwrap_or(p);
                if !p.is_empty() {
                    paths.insert(p.to_string());
                }
            }
        }
    }
    paths
}

/// Decide whether a proposed diff is safe to apply autonomously.
/// `allowed`: the diagnosed component file(s) the proposer was scoped to.
/// Returns Ok(()) if safe, Err(reason) otherwise. Default-deny.
pub fn check_diff_safety(diff: &str, allowed: &[&str]) -> Result<(), String> {
    let paths = diff_paths(diff);
    if paths.is_empty() {
        return Err("diff touches no parseable files (malformed or empty)".into());
    }
    for p in &paths {
        // 1. hard-forbidden zones — never, regardless of allow-list
        if let Some(z) = HARD_FORBIDDEN.iter().find(|z| p.contains(*z)) {
            return Err(format!("diff touches HARD-FORBIDDEN zone '{z}' (path {p})"));
        }
        // 2. default-deny: must be within the scoped allow-list
        if !allowed.iter().any(|a| p == a || p.starts_with(a)) {
            return Err(format!(
                "diff touches {p}, outside the scoped component allow-list {allowed:?}"
            ));
        }
        // 3. no test files (a change must pass the EXISTING tests, not rewrite them)
        if p.ends_with("_test.rs") || p.contains("/tests/") {
            return Err(format!("diff touches a test file {p} (must pass existing tests)"));
        }
    }
    // 4. body-level eval/test tampering signals
    for line in diff.lines() {
        if !line.starts_with('+') {
            continue; // only inspect added lines
        }
        if let Some(sig) = FORBIDDEN_BODY_SIGNALS.iter().find(|s| line.contains(*s)) {
            return Err(format!("diff adds a forbidden eval/test signal '{sig}'"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff_for(path: &str, body_add: &str) -> String {
        format!("--- a/{path}\n+++ b/{path}\n@@ -1 +1,2 @@\n unchanged\n+{body_add}\n")
    }

    #[test]
    fn allows_a_scoped_component_edit() {
        let d = diff_for("src/agentd/react.rs", "let t = 0.9;");
        assert!(check_diff_safety(&d, &["src/agentd/react.rs"]).is_ok());
    }

    #[test]
    fn denies_edit_outside_the_allow_list() {
        let d = diff_for("src/main.rs", "fn x() {}");
        assert!(check_diff_safety(&d, &["src/agentd/react.rs"]).is_err());
    }

    #[test]
    fn denies_touching_the_benchmark_fixtures() {
        let d = diff_for("scripts/benchmarks/repo_fix/fix_001/check.py", "sys.exit(0)");
        let err = check_diff_safety(&d, &["scripts/benchmarks/repo_fix/fix_001/check.py"]).unwrap_err();
        assert!(err.contains("HARD-FORBIDDEN"));
    }

    #[test]
    fn denies_touching_the_evaluator_and_safety() {
        for f in ["src/evolved/hiro.rs", "src/policyd/gating.rs", "src/evolved/code_safety.rs"] {
            let d = diff_for(f, "x");
            assert!(check_diff_safety(&d, &[f]).unwrap_err().contains("HARD-FORBIDDEN"));
        }
    }

    #[test]
    fn denies_test_rewrites_and_eval_signals() {
        let d = diff_for("src/agentd/react.rs", "#[cfg(test)]");
        assert!(check_diff_safety(&d, &["src/agentd/react.rs"]).is_err());
        let d2 = diff_for("src/agentd/react.rs", "repo_fix_measure(&o);");
        assert!(check_diff_safety(&d2, &["src/agentd/react.rs"]).is_err());
    }

    #[test]
    fn denies_empty_or_malformed_diff() {
        assert!(check_diff_safety("not a diff", &["src/agentd/react.rs"]).is_err());
    }
}
