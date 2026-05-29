# Nested brain/ — RUNTIME SCRATCH ONLY

This directory is **gitignored** (except for this STUB.md). Anything else written here is local-only and will not be tracked.

## Why this exists

The canonical research documents — hypotheses, knowledge base, paper outline, dead-ends, questions, inventions — live at **`brain/` in the repo root**, two levels up. Those are human-curated, citation-rich, and version-controlled.

The autonomous loop historically wrote here by accident when its `cwd` resolved to the Rust crate dir (`professor-x/professor-x/`). Those writes corrupted the nested copies and led to the May 24 reward-hacked status flips (see `docs/audits/INVALIDATED_COMMITS.md`).

## What goes where

| Kind of write | Location |
|---|---|
| Hypothesis statements, status changes, evidence | `brain/` (repo root) — gated by `--validate-artifacts` |
| Literature notes destined for the paper | `brain/` (repo root) — kind: `LiteratureNote` |
| Experiment results with run_id + harness_commit | `brain/` (repo root) — kind: `ExperimentResult` |
| Scratch reasoning, intermediate computations | `PROFESSOR_X_DATA_DIR` (default `~/.professor-x/`) |
| Anything that doesn't fit the schemas above | discard or escalate; do not write to either brain/ |

## What enforces this

`src/artifacts.rs::ArtifactValidator` rejects any write whose target path resolves into `professor-x/brain/*`. The check runs on every task with a declared `expected_artifact_kind`.

## When you might want to delete this file

Never, unless the nested directory itself is removed. The STUB is a tripwire: if it disappears, something deleted the directory enforcement.
