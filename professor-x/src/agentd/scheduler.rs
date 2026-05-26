/// Cron scheduler — schema and crash-safety pattern from Hermes Agent.
///
/// Key invariant from hermes-agent/cron/scheduler.py line 1829:
///   advance_next_run() is called BEFORE execution, under file lock.
///   This gives at-most-once semantics even if the process crashes mid-run.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScheduleType {
    Once,
    Interval,
    Cron,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobState {
    Scheduled,
    Paused,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub schedule_type: ScheduleType,
    /// For Interval: "7200" (seconds). For Cron: "0 6 * * *". For Once: ISO datetime.
    pub schedule_value: String,
    pub next_run_at: DateTime<Utc>,
    pub enabled: bool,
    pub state: JobState,
    pub repeat_limit: Option<u32>,
    pub repeat_completed: u32,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct CronScheduler {
    db: Arc<Mutex<Connection>>,
}

impl CronScheduler {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    pub fn register(&self, job: &CronJob) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO cron_jobs
             (id, name, prompt, schedule_type, schedule_value, next_run_at, enabled,
              state, repeat_limit, repeat_completed, last_run_at, last_status, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                job.id,
                job.name,
                job.prompt,
                format!("{:?}", job.schedule_type),
                job.schedule_value,
                job.next_run_at.to_rfc3339(),
                job.enabled as i32,
                format!("{:?}", job.state),
                job.repeat_limit.map(|n| n as i64),
                job.repeat_completed as i64,
                job.last_run_at.map(|t| t.to_rfc3339()),
                job.last_status.clone(),
                job.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn disable_legacy_daily_cycle(&self) -> Result<usize> {
        let db = self.db.lock().unwrap();
        let disabled = db.execute(
            "UPDATE cron_jobs
             SET enabled = 0, state = 'Paused', last_status = 'disabled: legacy daily cycle'
             WHERE enabled = 1
               AND (
                   id = 'daily-autonomous-cycle'
                   OR name = 'Daily research cycle'
                   OR prompt LIKE 'Run the daily autonomous research cycle:%'
               )",
            [],
        )?;
        Ok(disabled)
    }

    /// Called every 60 seconds. Returns jobs that are due.
    /// Advances next_run_at FIRST (Hermes crash-safety pattern).
    pub fn tick(&self) -> Result<Vec<CronJob>> {
        let now = Utc::now();
        let due_jobs = self.get_due_jobs(&now)?;

        // Advance next_run_at for ALL due jobs BEFORE any execution (at-most-once).
        for job in &due_jobs {
            self.advance_next_run(&job.id)?;
        }

        if !due_jobs.is_empty() {
            info!("scheduler: {} job(s) due at {}", due_jobs.len(), now.format("%H:%M:%S"));
        } else {
            debug!("scheduler tick: no jobs due");
        }

        Ok(due_jobs)
    }

    fn get_due_jobs(&self, now: &DateTime<Utc>) -> Result<Vec<CronJob>> {
        let db = self.db.lock().unwrap();
        let now_str = now.to_rfc3339();
        let mut stmt = db.prepare(
            "SELECT id, name, prompt, schedule_type, schedule_value, next_run_at, enabled,
                    state, repeat_limit, repeat_completed, last_run_at, last_status, created_at
             FROM cron_jobs
             WHERE enabled = 1 AND state = 'Scheduled' AND next_run_at <= ?1",
        )?;
        let rows = stmt.query_map(params![now_str], parse_job)?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }

    /// Advance next_run_at based on schedule type. Written before execution (crash safe).
    fn advance_next_run(&self, job_id: &str) -> Result<()> {
        let db = self.db.lock().unwrap();

        // Fetch current job to compute next run
        let (schedule_type, schedule_value): (String, String) = db.query_row(
            "SELECT schedule_type, schedule_value FROM cron_jobs WHERE id = ?1",
            params![job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let next = match schedule_type.as_str() {
            "Once" => {
                // Mark completed, no next run
                db.execute(
                    "UPDATE cron_jobs SET state = 'Completed', last_run_at = ?1 WHERE id = ?2",
                    params![Utc::now().to_rfc3339(), job_id],
                )?;
                return Ok(());
            }
            "Interval" => {
                let secs: i64 = schedule_value.parse().unwrap_or(3600);
                Utc::now() + chrono::Duration::seconds(secs)
            }
            "Cron" => {
                // Parse cron expression and find next fire time
                next_cron_time(&schedule_value).unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1))
            }
            _ => Utc::now() + chrono::Duration::hours(1),
        };

        db.execute(
            "UPDATE cron_jobs SET next_run_at = ?1, last_run_at = ?2,
                                  repeat_completed = repeat_completed + 1
             WHERE id = ?3",
            params![next.to_rfc3339(), Utc::now().to_rfc3339(), job_id],
        )?;
        Ok(())
    }
}

/// Compute next fire time for a cron expression string.
/// Uses the `cron` crate. Falls back to +1h on parse failure.
fn next_cron_time(expr: &str) -> Option<DateTime<Utc>> {
    use std::str::FromStr;
    let schedule = cron::Schedule::from_str(expr).ok()?;
    schedule.upcoming(Utc).next()
}

fn parse_job(row: &rusqlite::Row) -> rusqlite::Result<CronJob> {
    let next_run_at: String = row.get(5)?;
    let created_at: String = row.get(12)?;
    let last_run_at: Option<String> = row.get(10)?;

    Ok(CronJob {
        id: row.get(0)?,
        name: row.get(1)?,
        prompt: row.get(2)?,
        schedule_type: match row.get::<_, String>(3)?.as_str() {
            "Interval" => ScheduleType::Interval,
            "Cron" => ScheduleType::Cron,
            _ => ScheduleType::Once,
        },
        schedule_value: row.get(4)?,
        next_run_at: DateTime::parse_from_rfc3339(&next_run_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        enabled: row.get::<_, i32>(6)? != 0,
        state: match row.get::<_, String>(7)?.as_str() {
            "Paused" => JobState::Paused,
            "Completed" => JobState::Completed,
            "Error" => JobState::Error,
            _ => JobState::Scheduled,
        },
        repeat_limit: row.get::<_, Option<i64>>(8)?.map(|n| n as u32),
        repeat_completed: row.get::<_, i64>(9)? as u32,
        last_run_at: last_run_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))
        }),
        last_status: row.get(11)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}
