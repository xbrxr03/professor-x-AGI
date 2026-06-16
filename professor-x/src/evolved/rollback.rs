//! Rollback monitoring for accepted autonomous commits (evolution plan item #1, parts 2–3).
//!
//! When the operator accepts and applies a self-change it records an `applied_commit`
//! (`VerificationOutcome::applied_commit`). The plan asks: after the *next* run, did that change
//! actually hold, or did it have to be rolled back? This module answers that from git alone —
//! no model, no self-modification — so the verdict is cheap, deterministic, and surfaceable in
//! the status/observer views.
//!
//! Verdict:
//!   - `Held`     — the commit is reachable from HEAD and no later revert references it.
//!   - `Reverted` — the commit is in history but a later commit reverts it.
//!   - `Missing`  — the commit is unknown to this repo or not reachable from HEAD (never landed,
//!                  or a history rewrite dropped it) — treated as not-holding.

use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RollbackStatus {
    Held,
    Reverted,
    Missing,
}

impl RollbackStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Reverted => "reverted",
            Self::Missing => "missing",
        }
    }

    /// Did the accepted change survive (i.e. is it still in effect)?
    pub fn holds(self) -> bool {
        matches!(self, Self::Held)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RollbackVerdict {
    pub commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    pub present_in_head: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverted_by: Option<String>,
    pub status: RollbackStatus,
}

/// Pure classification so the policy is unit-testable without a git repo.
pub fn classify(present_in_head: bool, reverted: bool) -> RollbackStatus {
    if !present_in_head {
        RollbackStatus::Missing
    } else if reverted {
        RollbackStatus::Reverted
    } else {
        RollbackStatus::Held
    }
}

/// Compute the rollback verdict for an accepted `applied_commit` against the current HEAD.
pub async fn applied_commit_verdict(repo_root: &Path, commit: &str) -> Result<RollbackVerdict> {
    let resolved = git_rev_parse(repo_root, commit).await.ok();
    let present_in_head = if resolved.is_some() {
        git_is_ancestor(repo_root, commit).await?
    } else {
        false
    };
    let reverted_by = if present_in_head {
        let needle = resolved.as_deref().unwrap_or(commit);
        find_revert(repo_root, needle).await?
    } else {
        None
    };
    let status = classify(present_in_head, reverted_by.is_some());
    Ok(RollbackVerdict {
        commit: commit.to_string(),
        resolved,
        present_in_head,
        reverted_by,
        status,
    })
}

/// Synchronous variant for sync render paths (one-shot status/observer views). Same logic via
/// blocking `std::process` — cheap enough for a one-shot status document.
pub fn applied_commit_verdict_blocking(repo_root: &Path, commit: &str) -> RollbackVerdict {
    let run = |args: &[&str]| -> Option<std::process::Output> {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .ok()
    };
    let resolved = run(&["rev-parse", "--verify", "--quiet", &format!("{commit}^{{commit}}")])
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let present_in_head = resolved.is_some()
        && run(&["merge-base", "--is-ancestor", commit, "HEAD"])
            .map(|o| o.status.success())
            .unwrap_or(false);
    let reverted_by = if present_in_head {
        let needle = resolved.as_deref().unwrap_or(commit);
        run(&["log", "HEAD", "--format=%H", &format!("--grep=This reverts commit {needle}")])
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .find(|l| !l.is_empty())
            })
    } else {
        None
    };
    let status = classify(present_in_head, reverted_by.is_some());
    RollbackVerdict {
        commit: commit.to_string(),
        resolved,
        present_in_head,
        reverted_by,
        status,
    }
}

/// Resolve a (possibly short) ref to a full commit hash; `Err` if git does not know it.
async fn git_rev_parse(repo_root: &Path, commit: &str) -> Result<String> {
    let out = tokio::process::Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("{commit}^{{commit}}")])
        .current_dir(repo_root)
        .output()
        .await?;
    if !out.status.success() {
        anyhow::bail!("unknown commit {commit}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Is `commit` reachable from HEAD? (A commit is its own ancestor, so HEAD itself counts.)
async fn git_is_ancestor(repo_root: &Path, commit: &str) -> Result<bool> {
    let out = tokio::process::Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, "HEAD"])
        .current_dir(repo_root)
        .output()
        .await?;
    Ok(out.status.success())
}

/// Find a later commit that reverts `full_hash`. `git revert` embeds the full hash in its
/// message ("This reverts commit <hash>."), so we grep the log for it.
async fn find_revert(repo_root: &Path, full_hash: &str) -> Result<Option<String>> {
    let out = tokio::process::Command::new("git")
        .args([
            "log",
            "HEAD",
            "--format=%H",
            &format!("--grep=This reverts commit {full_hash}"),
        ])
        .current_dir(repo_root)
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .find(|l| !l.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_covers_all_three_verdicts() {
        assert_eq!(classify(false, false), RollbackStatus::Missing);
        assert_eq!(classify(false, true), RollbackStatus::Missing); // not present dominates
        assert_eq!(classify(true, true), RollbackStatus::Reverted);
        assert_eq!(classify(true, false), RollbackStatus::Held);
        assert!(RollbackStatus::Held.holds());
        assert!(!RollbackStatus::Reverted.holds());
        assert!(!RollbackStatus::Missing.holds());
    }

    async fn git(dir: &Path, args: &[&str]) {
        let out = tokio::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .await
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    #[tokio::test]
    async fn held_then_reverted_then_missing_verdicts() {
        let dir = std::env::temp_dir().join(format!("px-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        std::fs::write(dir.join("a.txt"), "1").unwrap();
        git(&dir, &["add", "."]).await;
        git(&dir, &["commit", "-qm", "base"]).await;
        std::fs::write(dir.join("b.txt"), "feature").unwrap();
        git(&dir, &["add", "."]).await;
        git(&dir, &["commit", "-qm", "the change"]).await;
        let target = String::from_utf8_lossy(
            &tokio::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&dir)
                .output()
                .await
                .unwrap()
                .stdout,
        )
        .trim()
        .to_string();

        // Held: the change is HEAD, no revert.
        let v = applied_commit_verdict(&dir, &target).await.unwrap();
        assert_eq!(v.status, RollbackStatus::Held, "{v:?}");

        // Reverted: revert the change, verdict flips.
        git(&dir, &["revert", "--no-edit", &target]).await;
        let v = applied_commit_verdict(&dir, &target).await.unwrap();
        assert_eq!(v.status, RollbackStatus::Reverted, "{v:?}");
        assert!(v.reverted_by.is_some());

        // Missing: a commit hash this repo never had.
        let v = applied_commit_verdict(&dir, "0000000000000000000000000000000000000000")
            .await
            .unwrap();
        assert_eq!(v.status, RollbackStatus::Missing, "{v:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
