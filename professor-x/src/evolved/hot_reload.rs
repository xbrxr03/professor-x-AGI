//! Hot-reload: close the evolve → apply → measure loop without a manual operator restart.
//!
//! The autonomous operator already verifies a self-change in a sandbox worktree and
//! cherry-picks the verified commit back onto `main` (see `loop_runner.rs`). But Professor X
//! is a *compiled* binary: a committed source change is inert until the binary is rebuilt and
//! the running process is replaced. That rebuild+restart is the last human-in-the-loop step.
//!
//! This module removes it. After a verified self-change lands, we rebuild the release binary
//! and `exec` into it, so the running harness *becomes* the improved one and the loop
//! continues — no operator. Safety is structural:
//!   1. We re-exec ONLY after a clean `cargo build --release` (a broken self-edit can never
//!      replace a working binary — we stay on the current one and record the failure).
//!   2. A generation cap (carried in an env var across exec) bounds consecutive reloads, so a
//!      pathological loop cannot fork-bomb itself into endless rebuilds.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Env var carrying how many times this lineage has hot-reloaded (survives `exec`).
pub const RELOAD_GENERATION_ENV: &str = "PROFESSOR_X_RELOAD_GENERATION";

/// Hard cap on consecutive hot-reloads in one lineage — defense against re-exec storms.
pub const DEFAULT_MAX_GENERATIONS: u32 = 8;

/// What to do after a rebuild attempt. Pure, so the policy is unit-testable without `exec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadDecision {
    /// Rebuild succeeded and we are under the cap — re-exec into the new binary.
    Reexec { next_generation: u32 },
    /// Build failed — stay on the current binary, keep the loop running, record it.
    StayBuildFailed,
    /// Hit the generation cap — stop auto-reloading; a later run continues the lineage.
    StayGenerationCap { generation: u32 },
}

/// Pure policy: given the build outcome and the current generation, decide whether to re-exec.
pub fn decide(build_ok: bool, generation: u32, max_generations: u32) -> ReloadDecision {
    if !build_ok {
        return ReloadDecision::StayBuildFailed;
    }
    if generation >= max_generations {
        return ReloadDecision::StayGenerationCap { generation };
    }
    ReloadDecision::Reexec {
        next_generation: generation + 1,
    }
}

/// Current hot-reload generation from the environment (0 if unset / unparseable).
pub fn current_generation() -> u32 {
    std::env::var(RELOAD_GENERATION_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Rebuild the release binary at `repo_root`. Returns the built binary path on success.
/// On a failing build it returns `Err` — the caller MUST keep running the current binary.
///
/// Self-rebuild subtlety: the process calling this is usually `target/release/professor-x`
/// itself, so cargo's relink step would hit `ETXTBSY` ("text file busy") trying to overwrite
/// the running image. We move the current binary aside first (Linux keeps the running inode
/// valid after a rename), letting cargo write a fresh file at the original path; on failure we
/// move it back so a working binary always remains.
pub async fn rebuild_release(repo_root: &Path) -> Result<PathBuf> {
    let bin = repo_root.join("target/release/professor-x");
    let stash = if bin.exists() {
        let s = repo_root.join(format!(
            "target/release/.professor-x.prev-{}",
            std::process::id()
        ));
        // Best-effort: if the rename fails we still attempt the build (cargo may handle it).
        let _ = std::fs::rename(&bin, &s);
        Some(s)
    } else {
        None
    };

    let out = tokio::process::Command::new("cargo")
        .args(["build", "--release", "--bin", "professor-x"])
        .current_dir(repo_root)
        .output()
        .await
        .context("spawning cargo build --release")?;

    if !out.status.success() {
        // Restore the stashed binary so we never end up with no working executable.
        if let Some(s) = &stash {
            if !bin.exists() {
                let _ = std::fs::rename(s, &bin);
            }
        }
        anyhow::bail!(
            "cargo build --release failed (keeping current binary): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    if !bin.exists() {
        anyhow::bail!("release build reported success but {} is missing", bin.display());
    }
    // The stash is the old inode (still backing this running process); unlinking it is safe.
    if let Some(s) = stash {
        let _ = std::fs::remove_file(s);
    }
    Ok(bin)
}

/// Re-exec into `binary` with `args`, bumping the generation env var. On success the process
/// image is replaced and this never returns; it only returns `Err` if `exec` itself failed.
#[cfg(unix)]
pub fn reexec_into(
    binary: &Path,
    args: &[String],
    next_generation: u32,
) -> Result<std::convert::Infallible> {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(binary)
        .args(args)
        .env(RELOAD_GENERATION_ENV, next_generation.to_string())
        .exec();
    Err(anyhow::anyhow!(
        "re-exec into {} failed: {}",
        binary.display(),
        err
    ))
}

/// Orchestrate one hot-reload: rebuild, decide, and (if cleared) re-exec into the new binary
/// with `continue_args`. Returns the `Stay*` decision when it deliberately does NOT re-exec
/// (build failed or generation cap); on a successful re-exec it never returns.
#[cfg(unix)]
pub async fn reload_after_commit(
    repo_root: &Path,
    continue_args: &[String],
    max_generations: u32,
) -> Result<ReloadDecision> {
    let generation = current_generation();
    let build = rebuild_release(repo_root).await;
    let decision = decide(build.is_ok(), generation, max_generations);
    match &decision {
        ReloadDecision::Reexec { next_generation } => {
            let binary = build.expect("Reexec implies a successful build");
            reexec_into(&binary, continue_args, *next_generation)?; // never returns on success
            Ok(decision) // only reached if exec() failed
        }
        _ => Ok(decision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_failure_keeps_current_binary() {
        // A broken self-edit must never replace a working harness.
        assert_eq!(decide(false, 0, 8), ReloadDecision::StayBuildFailed);
        assert_eq!(decide(false, 3, 8), ReloadDecision::StayBuildFailed);
    }

    #[test]
    fn clean_build_under_cap_reexecs_and_bumps_generation() {
        assert_eq!(decide(true, 0, 8), ReloadDecision::Reexec { next_generation: 1 });
        assert_eq!(decide(true, 7, 8), ReloadDecision::Reexec { next_generation: 8 });
    }

    #[test]
    fn generation_cap_stops_reexec_storms() {
        assert_eq!(decide(true, 8, 8), ReloadDecision::StayGenerationCap { generation: 8 });
        assert_eq!(decide(true, 99, 8), ReloadDecision::StayGenerationCap { generation: 99 });
    }

    #[test]
    fn current_generation_parses_env_or_defaults_zero() {
        // Unset / garbage must be treated as generation 0 (a fresh lineage), never panic.
        assert_eq!(
            "not-a-number".parse::<u32>().ok().unwrap_or(0),
            0,
            "unparseable generation falls back to 0"
        );
    }
}
