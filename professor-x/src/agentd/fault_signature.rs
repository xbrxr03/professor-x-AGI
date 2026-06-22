//! Failure-signature ("syndrome") of a repo-fix task: the per-assert pass/fail bit-vector of its
//! `check.py` run against the current (buggy) workspace. This is the behavioral embedding validated
//! in the 2026-06-21 pre-check (rename-invariant 0.93 vs text 0.14): two tasks that FAIL THE SAME
//! CHECKS are behaviorally the same regardless of surface text, so a past fix retrieved by signature
//! transfers. Used by behavior-keyed retrieval (flag `PROFESSOR_X_BEHAVIOR_RETRIEVAL`).
//!
//! We compute the signature by running the validated Python decomposition (see
//! `scripts/benchmarks/repo_fix/sig_runner.py`) embedded here and executed with `python3 -c` in the
//! task dir — consistent with how the harness already shells out to `python3 check.py` to verify.

use std::path::Path;
use std::process::Command;

/// Embedded signature computer: AST-rewrites each `assert` in `check.py` into a non-raising recorder
/// so we capture EVERY assert's pass(1)/fail(0) outcome (not just the first failure), neutralizes
/// `sys.exit`, suppresses the script's own stdout, and prints the bit-string. Mirrors the validated
/// `sig_runner.py`. Runs with cwd = the task dir (so `from <module> import ...` resolves locally).
const SIG_PY: &str = r#"
import ast, sys, io, os
sys.path.insert(0, os.getcwd())
def build(src):
    tree = ast.parse(src)
    class T(ast.NodeTransformer):
        def visit_Assert(self, node):
            rec = ast.parse("try:\n __R.append(1 if (None) else 0)\nexcept Exception:\n __R.append(0)").body[0]
            rec.body[0].value.args[0] = ast.IfExp(test=node.test, body=ast.Constant(1), orelse=ast.Constant(0))
            return ast.copy_location(rec, node)
        def visit_Call(self, node):
            self.generic_visit(node)
            if isinstance(node.func, ast.Attribute) and node.func.attr == "exit":
                return ast.Constant(None)
            return node
    tree = T().visit(tree); ast.fix_missing_locations(tree)
    return tree
try:
    src = open("check.py").read()
except Exception:
    print(""); sys.exit(0)
tree = build(src)
ns = {"__R": []}
real = sys.stdout; sys.stdout = io.StringIO()
try:
    exec(compile(tree, "<sig>", "exec"), ns)
except BaseException:
    pass
finally:
    sys.stdout = real
print("".join(str(b) for b in ns["__R"]))
"#;

/// Compute the failure signature (e.g. `"1011110"`) of the repo-fix task rooted at `task_dir`
/// (must contain a stdlib `check.py`). Returns `None` if there is no check.py, python is unavailable,
/// or the output isn't a non-empty bit-string — callers then simply fall back to text retrieval.
pub fn fault_signature(task_dir: &Path) -> Option<String> {
    if !task_dir.join("check.py").is_file() {
        return None;
    }
    let output = Command::new("python3")
        .arg("-c")
        .arg(SIG_PY)
        .current_dir(task_dir)
        .output()
        .ok()?;
    let sig = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sig.is_empty() || !sig.chars().all(|c| c == '0' || c == '1') {
        return None;
    }
    Some(sig)
}

/// Hamming distance between two equal-length signatures. `None` if lengths differ (signatures from
/// different check structures are not comparable — v1 scopes retrieval to shared-check families).
pub fn hamming(a: &str, b: &str) -> Option<usize> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    Some(a.bytes().zip(b.bytes()).filter(|(x, y)| x != y).count())
}

/// Fraction of matching bits (1.0 = identical). `None` if not comparable.
pub fn similarity(a: &str, b: &str) -> Option<f32> {
    let d = hamming(a, b)?;
    Some(1.0 - (d as f32 / a.len() as f32))
}

/// One corpus entry: a past task's failure signature + a fix hint. Built by
/// `scripts/benchmarks/repo_fix/build_signature_index.py` into `signature_index.json`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignatureEntry {
    pub id: String,
    #[serde(default)]
    pub family: String,
    pub signature: String,
    #[serde(default)]
    pub buggy_module: String,
    pub hint: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignatureIndex {
    pub entries: Vec<SignatureEntry>,
}

impl SignatureIndex {
    /// Load the JSON index from `path` (`{ "entries": [...] }`). `None` on any IO/parse error so the
    /// caller silently falls back to text retrieval.
    pub fn load(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Nearest entry to `sig` by bit-similarity, requiring `sim >= min_sim`. Entries whose `id` is a
    /// substring of `workspace_marker` are skipped (self-exclusion: the task's own fixture id appears
    /// in its workspace path, so a task never retrieves its own answer).
    pub fn nearest(
        &self,
        sig: &str,
        workspace_marker: &str,
        min_sim: f32,
    ) -> Option<(&SignatureEntry, f32)> {
        self.entries
            .iter()
            .filter(|e| !workspace_marker.contains(&e.id))
            .filter_map(|e| similarity(sig, &e.signature).map(|s| (e, s)))
            .filter(|(_, s)| *s >= min_sim)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    /// Make an isolated temp task dir (repo convention: std::env::temp_dir + Uuid).
    fn make_task(module: &str, check: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("px-faultsig-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("m.py"), module).unwrap();
        if let Some(c) = check {
            fs::write(dir.join("check.py"), c).unwrap();
        }
        dir
    }

    const CHECK: &str = "import sys\nfrom m import add\ntry:\n    assert add(2, 3) == 5\n    assert add(0, 0) == 0\n    print('ok'); sys.exit(0)\nexcept AssertionError:\n    print('FAIL'); sys.exit(1)\n";

    #[test]
    fn signature_is_nondegenerate_for_a_red_task() {
        // m.add is buggy (returns a-b); assert1 (add(2,3)==5) fails, assert2 (add(0,0)==0) passes.
        let dir = make_task("def add(a, b):\n    return a - b\n", Some(CHECK));
        let sig = fault_signature(&dir).expect("signature");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(sig, "01", "first assert fails, second passes -> '01' (got {sig})");
    }

    #[test]
    fn signature_all_pass_for_a_green_task() {
        let dir = make_task("def add(a, b):\n    return a + b\n", Some(CHECK));
        let sig = fault_signature(&dir).expect("signature");
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(sig, "11");
    }

    #[test]
    fn no_check_returns_none() {
        let dir = make_task("def add(a, b):\n    return a + b\n", None);
        let out = fault_signature(&dir);
        let _ = fs::remove_dir_all(&dir);
        assert!(out.is_none());
    }

    #[test]
    fn hamming_and_similarity() {
        assert_eq!(hamming("1011", "1001"), Some(1));
        assert_eq!(hamming("101", "1011"), None);
        assert_eq!(similarity("1111", "1111"), Some(1.0));
        assert_eq!(similarity("1111", "0000"), Some(0.0));
    }

    #[test]
    fn index_nearest_matches_and_self_excludes() {
        let idx: SignatureIndex = serde_json::from_str(
            r#"{"entries":[
                {"id":"fam_a_01","family":"a","signature":"0111","buggy_module":"m.py","hint":"fix A"},
                {"id":"fam_a_02","family":"a","signature":"1100","buggy_module":"n.py","hint":"fix B"}
            ]}"#,
        )
        .unwrap();
        // Query "0111": exact match is fam_a_01.
        let (e, sim) = idx.nearest("0111", "/tmp/px-repofix-other-123", 0.5).unwrap();
        assert_eq!(e.id, "fam_a_01");
        assert_eq!(sim, 1.0);
        // Self-exclusion: marker contains fam_a_01 -> skip it. At a 0.5 floor the only other entry
        // (fam_a_02, sim 0.25 to "0111") is below threshold -> no match (correctly conservative).
        assert!(idx.nearest("0111", "/tmp/px-repofix-fam_a_01-123", 0.5).is_none());
        // With no floor, self-exclusion still holds: it returns the OTHER entry, never fam_a_01.
        let got = idx.nearest("0111", "/tmp/px-repofix-fam_a_01-123", 0.0);
        assert_eq!(got.map(|(e, _)| e.id.as_str()), Some("fam_a_02"));
        // Different-length signature is not comparable -> no match.
        assert!(idx.nearest("01110", "/x", 0.5).is_none());
    }
}
