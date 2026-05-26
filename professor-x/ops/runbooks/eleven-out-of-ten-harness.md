# 11/10 Rust Harness Roadmap

Professor X should become a harness that can stand beside the strongest public agent systems, then exceed them by being local-first, self-measuring, self-evolving, and rollback-safe.

## Reference Systems

The repo research materials name these systems as the comparison set:

| System | What We Must Match Or Beat |
| --- | --- |
| Codex / Claude Code / OpenHands | High-quality coding loop, patch discipline, terminal UX, repo awareness, test iteration |
| ClawOS | Policy gate, audit chain, user-visible security posture |
| Hermes | Persistent memory, scheduled autonomy, resumable jobs |
| OpenClaw | `SKILL.md` compatibility and large skill ecosystem |
| Voyager | Verified growing skill library and skill reuse |
| AutoGen / MetaGPT | Multi-agent role decomposition and controlled termination |
| AIOS | Harness-as-operating-system framing: memory, tools, scheduling, context allocation |
| AHE | Component observability, change manifests, harness-evolution measurement |
| MOSS | Source-level verify-then-commit harness rewriting |
| Meta-Harness / HyperAgents | Strong harness optimization baselines and improvement-at-k framing |
| ASI-Evolve | Researcher / Engineer / Analyzer loop and cognition base |
| EvolveR / AgentEvolver / WebEvolver | Reflective self-improvement and self-evolving agent methodology |
| Scientific-agent skill repos | Research workflow skills, artifact reproducibility, scientific writing discipline |

## Current Rating

Current harness: **3.5/10** relative to Codex as 10/10.

What works:
- Rust single-binary harness with `memd`, `toolbridge`, `agentd`, `policyd`, and `evolved`.
- Workspace-bound policy gate and Merkle audit chain.
- ReAct loop, scheduler, daily jobs, skills, memory, HIRO runner, reward-hacking checks, and evolution scaffolding.
- Live observer UI and event stream via `--lab`, `--observe`, `--events`, `--status`, and `--watch`.

Why it is not yet production-grade:
- Tool execution is not yet as ergonomic or reliable as a coding CLI.
- The agent can still produce low-quality artifacts without enough semantic validation.
- Verify-then-commit evolution is not strict enough for unattended Rust self-modification.
- There is no mature sandbox/worktree accept-reject pipeline for proposals.
- DHE/BF/LCAP are partially operational but not yet tightly coupled to every failure and proposal.
- Skills are loaded but not yet a strong reusable execution substrate.
- No long-running run has proven stable daily operation.

## 11/10 Definition

Professor X reaches 11/10 when all of these are true:

1. **Observable:** every task, model response, tool call, policy decision, artifact, benchmark, proposal, commit, and rollback is visible in `--lab` and persisted as DB + JSONL.
2. **Safe:** every tool and autonomous code path is workspace-bound, audited, kill-switchable, and rollback-safe.
3. **Useful:** the daily loop produces real research artifacts with source-grounded evidence, not placeholder files.
4. **Measurable:** HIRO attempt data, null baselines, run ids, commit ids, and variance are recorded before crediting improvements.
5. **Codex-grade coding:** patch proposals are minimal, tested, reviewed by automated checks, and never silently overwrite unrelated changes.
6. **Verify-then-commit:** every autonomous repo change happens in an isolated branch/worktree, passes checks, passes reward-hacking scan, then commits or rolls back.
7. **Diagnostic:** every failure produces a DHE trace and every proposal cites the trace it targets.
8. **Adaptive:** BF and LCAP steer future tasks/proposals from measured round-level evidence.
9. **Skillful:** verified skills can be reused without wasting LLM calls on routine work.
10. **Scientific:** research claims require artifacts, metrics, run ids, and falsifiable hypotheses.
11. **Self-evolving:** after the above gates are proven, Prof X can safely improve Rust core modules and revert regressions automatically.

## Execution Phases

### Phase A: Codex-Grade Tool Loop

Goal: make one-shot and scheduled tasks feel like a real coding agent.

- Add structured task transcripts: thought/action/observation, file diffs, command outputs, exit codes, duration, and artifact links.
- Add first-class patch application and diff review helpers instead of relying on generic shell writes.
- Add command output truncation with full artifact capture.
- Add `--chat` or `--task-interactive` for conversational tasking from the terminal.
- Add task cancellation, pause/resume, and visible failure classification.

Acceptance:
- A user can give a repo task, watch each step in `--lab`, inspect the generated diff, and see why the task passed or failed.

### Phase B: Artifact Truth Layer

Goal: stop fake progress.

- Define artifact schemas for daily updates, literature notes, experiments, HIRO runs, proposals, rejections, and paper drafts.
- Validate dates, paths, source citations, run ids, commit ids, and required fields.
- Mark tasks failed if expected artifacts are missing, misplaced, stale, or unsupported.
- Add artifact links to observer selected-event payloads.

Acceptance:
- Bad artifacts like nested `professor-x/professor-x/...` or fake dated notes fail automatically and are visible in `--lab`.

### Phase C: Verify-Then-Commit Evolution Gate

Goal: Prof X may propose changes, but only verified changes can land.

- Create temporary worktree or branch per proposal.
- Apply proposed diff only inside allowed target components.
- Run `cargo check`, targeted tests, HIRO subset, policy regression tests, and reward-hacking scan.
- Compare against stored baseline.
- Commit accepted changes and record an evolution node; reject and delete the worktree otherwise.
- Add automatic rollback monitor for the next full cycle.

Acceptance:
- A bad proposal is rejected and rolled back.
- A no-op/comment-only proposal is not credited.
- A safe accepted proposal maps to a commit and an evolution node.

### Phase D: HIRO + DHE + BF + LCAP Coupling

Goal: make improvement evidence scientific.

- Ensure every failed HIRO task writes a `DiagnosticTrace`.
- Require `ChangeManifest.root_cause` to cite DHE evidence.
- Use BF category regressions to choose what to debug next.
- Update LCAP only from round-level evidence, not single-task noise.
- Add observer panels for HIRO trend, BF vector, DHE attribution counts, and LCAP arm state.

Acceptance:
- Any claimed improvement has a task id, category, commit id, metric delta, and diagnostic reason.

### Phase E: Skill Runtime

Goal: make skills real execution units.

- Normalize skills into `SKILL.md` directories or make the loader intentionally support current markdown layout.
- Let skills declare allowed tools, artifact schemas, expected outputs, and verification commands.
- Track skill outcomes and retire bad or duplicate skills.
- Add non-LLM fast paths for routine verified skills.

Acceptance:
- A daily job can load a skill, execute it with the right permission scope, validate its artifact, and update skill quality.

### Phase F: Long-Run Operations

Goal: Prof X can stay alive without corrupting the repo.

- Add `systemd` or local supervisor runbook.
- Add heartbeat events, idle detection, self-termination after repeated idle cycles, and restartable state.
- Add daily GitHub-visible summaries after validation.
- Add alert events for policy denials, artifact failures, audit-chain failure, benchmark regression, and rollback.

Acceptance:
- A 24-hour run creates valid logs/artifacts, stops cleanly, and can resume.

## Immediate Next Move

Implement **Phase A + B foundation**:

1. Add task transcript persistence.
2. Add artifact schema definitions and validators.
3. Surface transcript/artifact links in `--lab`.
4. Run a short `--lab --run-now` session and use the failures to drive the verify-then-commit gate.

This is the shortest path from an observable daemon to a harness that can safely become self-evolving.
