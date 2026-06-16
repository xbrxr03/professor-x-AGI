//! Autonomy health & alerts (evolution plan item #5).
//!
//! A long unattended supervised run must surface trouble instead of silently degrading. This
//! aggregates recent work-loop runs into a health verdict + alerts that a daemon/monitor (or the
//! status view) can act on: cycle pass rate, a streak of failing runs, and a Healthy/Degraded/
//! Unhealthy status. Pure over the run records, so it is deterministic and unit-testable.

use super::work_loops::WorkLoopRunRecord;

/// Alert thresholds. Kept as constants so the policy is explicit and testable.
const MIN_HEALTHY_PASS_RATE: f32 = 0.80;
const MIN_DEGRADED_PASS_RATE: f32 = 0.50;
const UNHEALTHY_FAILED_STREAK: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutonomyHealth {
    pub runs: usize,
    pub total_cycles: u32,
    pub passed_cycles: u32,
    pub failed_cycles: u32,
    pub pass_rate: f32,
    pub consecutive_failed_runs: u32,
    pub status: HealthStatus,
    pub alerts: Vec<String>,
}

/// Summarize autonomy health from recent runs. `runs` must be most-recent-first, as
/// `WorkLoopRunStore::recent` returns.
pub fn summarize_autonomy_health(runs: &[WorkLoopRunRecord]) -> AutonomyHealth {
    let passed_cycles: u32 = runs.iter().map(|r| r.passed_cycles).sum();
    let failed_cycles: u32 = runs.iter().map(|r| r.failed_cycles).sum();
    let total_cycles = passed_cycles + failed_cycles;
    let pass_rate = if total_cycles == 0 {
        0.0
    } else {
        passed_cycles as f32 / total_cycles as f32
    };

    // A run "failed" if any of its cycles failed. Count the streak from the most recent run.
    let consecutive_failed_runs = runs
        .iter()
        .take_while(|r| r.failed_cycles > 0)
        .count() as u32;

    let mut alerts = Vec::new();
    if runs.is_empty() {
        alerts.push("no autonomy runs recorded".to_string());
    }
    if total_cycles > 0 && pass_rate < MIN_HEALTHY_PASS_RATE {
        alerts.push(format!(
            "cycle pass rate {pass_rate:.2} below {MIN_HEALTHY_PASS_RATE:.2}"
        ));
    }
    if consecutive_failed_runs >= UNHEALTHY_FAILED_STREAK {
        alerts.push(format!(
            "{consecutive_failed_runs} consecutive runs with cycle failures"
        ));
    }

    let status = if consecutive_failed_runs >= UNHEALTHY_FAILED_STREAK
        || (total_cycles > 0 && pass_rate < MIN_DEGRADED_PASS_RATE)
    {
        HealthStatus::Unhealthy
    } else if !alerts.is_empty() || consecutive_failed_runs >= 1 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    AutonomyHealth {
        runs: runs.len(),
        total_cycles,
        passed_cycles,
        failed_cycles,
        pass_rate,
        consecutive_failed_runs,
        status,
        alerts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn run(passed: u32, failed: u32) -> WorkLoopRunRecord {
        WorkLoopRunRecord {
            id: None,
            run_id: "r".to_string(),
            run_kind: "operator".to_string(),
            profile: "core".to_string(),
            started_at: Utc::now(),
            completed_at: Utc::now(),
            requested_cycles: passed + failed,
            completed_cycles: passed + failed,
            passed_cycles: passed,
            failed_cycles: failed,
            report_path: String::new(),
            planned_jobs: Vec::new(),
            smoke_records: Vec::new(),
            recorded_at: Utc::now(),
        }
    }

    #[test]
    fn all_passing_runs_are_healthy() {
        let h = summarize_autonomy_health(&[run(6, 0), run(4, 0)]);
        assert_eq!(h.status, HealthStatus::Healthy);
        assert!(h.alerts.is_empty());
        assert!((h.pass_rate - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_recent_failing_run_degrades() {
        // 9/10 cycles pass, but the most recent run had a failure → degraded, not unhealthy.
        let h = summarize_autonomy_health(&[run(0, 1), run(9, 0)]);
        assert_eq!(h.status, HealthStatus::Degraded);
        assert_eq!(h.consecutive_failed_runs, 1);
    }

    #[test]
    fn a_streak_of_failing_runs_is_unhealthy_and_alerts() {
        let h = summarize_autonomy_health(&[run(0, 2), run(1, 1), run(0, 3), run(5, 0)]);
        assert_eq!(h.status, HealthStatus::Unhealthy);
        assert_eq!(h.consecutive_failed_runs, 3);
        assert!(h.alerts.iter().any(|a| a.contains("consecutive")));
    }

    #[test]
    fn no_runs_alerts_but_is_not_unhealthy() {
        let h = summarize_autonomy_health(&[]);
        assert_eq!(h.runs, 0);
        assert!(h.alerts.iter().any(|a| a.contains("no autonomy runs")));
        assert_ne!(h.status, HealthStatus::Unhealthy);
    }
}
