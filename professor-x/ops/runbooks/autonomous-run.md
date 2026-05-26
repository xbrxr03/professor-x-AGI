# Autonomous Run Runbook

This is the local-first path to an autonomous Professor X run. The repo must be clean before autonomous evolution can apply or commit changes.

## Readiness

From the repository root:

```bash
scripts/autonomy-readiness.sh
```

The readiness script verifies:

- Rust compile checks pass.
- Rust tests pass.
- The daily-cycle schedule parses and loads project skills.
- The working tree status is visible before launch.

## Static Baseline

Run a fast smoke baseline first:

```bash
cd professor-x
PROFESSOR_X_DATA_DIR=/tmp/px-hiro-smoke cargo run -- --hiro 0 --hiro-limit 1
```

Then run a null-condition baseline before crediting any autonomous change:

```bash
cd professor-x
PROFESSOR_X_DATA_DIR="$PWD/.px-data-null" cargo run -- --hiro-null 3
```

For a faster null-condition smoke run, add `--hiro-limit N`.

Record the resulting run id, harness commit, and HIRO metrics before starting evolution.

## Start Daily Autonomous Operation

```bash
cd professor-x
cargo run -- --run-now
```

`--run-now` schedules the seven explicit daily jobs from `ops/schedules/daily-cycle.toml` starting immediately. Without `--run-now`, the same jobs start at the next 22:00 UTC daily cycle and repeat every 24 hours.

## Observe Prof X

For the normal “agent lab” experience, start the daemon and observer together:

```bash
cd professor-x
cargo run -- --lab --run-now
```

Use `q`, Esc, or Ctrl+C to close the observer and stop the daemon cleanly.

Open the full-screen terminal observer in a second shell:

```bash
cd professor-x
cargo run -- --observe
```

Useful inspection commands:

```bash
cargo run -- --status
cargo run -- --events 25
cargo run -- --watch
```

The observer follows the same durable event stream that is written to SQLite and mirrored as JSONL under `artifacts/events/`. It shows scheduler state, audit counts, HIRO status, recent task/tool/policy/evolution activity, a live event timeline, and the selected event payload.

## Inspect Evidence

Every completed task writes a durable transcript:

```bash
find artifacts/transcripts -type f | sort
```

Scheduled jobs also write artifact-validation reports:

```bash
find artifacts/validation -type f | sort
```

Transcript and validation paths are emitted as `transcript.written`, `artifact.valid`, or `artifact.invalid` events, so they are visible from `--lab`, `--events`, and `--status`.

Patch artifacts from the reviewable patch tool are stored here:

```bash
find artifacts/patches -type f | sort
```

Autonomous coding tasks should prefer `patch.apply` with `mode=check` before `mode=apply`, then run compile/tests before any commit.

## Kill Switch

Use Ctrl+C for foreground runs, or send SIGUSR2 to the process for a graceful shutdown.

## Evolution Gate

Autonomous evolution is allowed to proceed only when:

- The working tree is clean.
- The proposal has a recorded target and provenance.
- Compile and selected HIRO checks pass.
- Reward-hacking checks do not flag the diff.
- Rejected or failed proposals are rolled back.
