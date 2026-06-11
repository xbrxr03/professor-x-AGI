# NEXT STEPS — ordered to-do (read before starting any work)

**North star:** `MILESTONE.md` (read it first). **Plan detail:**
`docs/research/2026-06-10-frankenstein-harness-master-plan.md`.
This file is the *execution order*. The milestone we're driving to: ship a daily-driver
**offline local coding agent people actually try (#2)**, then make it **improve itself
on real usage (#1)**. The goal everything serves: an honest, externally-credible number
+ a one-command try-it — not more features.

> **RE-AIMED 2026-06-10.** The edit stack (Phase 1.1–1.4 below) is built but the
> taxonomy showed it was NOT the bottleneck. **The current execution order is
> `M0 → M1 → M2` (below), which supersedes the remaining Phase 1.5 / 2 / 3 / 4 / 5
> polish.** Do M0 next. Phases 1.5–5 are deferred until M2's gates pass.

## Rules for agents (do not violate)
1. **Do tasks in number order.** Do NOT start a task whose `Blocked by` is unfinished.
2. **One task in progress at a time.** Mark `[~]` when you start, `[x]` when done with
   the commit hash. Never mark `[x]` unless its *Done-when* is literally true.
3. **Branch:** work from `main`. The `harness-gaps` research branch has been merged;
   keep future work on the single clean tree unless a specific PR branch is needed.
4. **Every build task ends with a measurement**, not a vibe. If you can't measure it,
   it's not done.
5. **Local-first.** Never add multi-provider/frontier-API reach. Ollama only.
6. **Reference source, don't reinvent:** cloned harnesses are in `_refs/harnesses/`
   (jcode, codex, goose, aider, SWE-agent, OpenHands, cline). File paths are cited in
   the master plan. codex (Rust) ports most directly.
7. If a task's premise turns out false (e.g. taxonomy says bad-edits are NOT the main
   failure), STOP and update this file + the plan before continuing.

---

# ★ CURRENT PRIORITY — M0 → M1 → M2 (do these next; supersede everything below)

See `MILESTONE.md` for the full ladder. Every gate is a number, not a vibe.

## M0 — Trust the scoreboard (DO THIS NEXT, gates everything)
*Why: the 0.5.3 re-measure was `p_correct=0.000` AND `p_plan=0.000` even after the
answer-gate fix. A metric that reads 0 after the fix, and a `p_plan` that never fires,
is degenerate — we cannot trust the number, and this project has shipped fabricated
metrics before. No new features until the scoreboard is honest.*
- [x] **M0.1 Diagnose the degenerate metrics.** For the 0.5.3 run, determine per task
  whether `p_correct=0` is (a) the judge rejecting a correct answer, (b) a genuinely
  wrong/absent answer, or (c) a scoring bug. Same for `p_plan` never firing.
  **Done-when:** a written cause per metric in `docs/research/eval-trust.md`.
  **Blocked by:** nothing.
  **Result:** `docs/research/eval-trust.md`. TWO causes, both = (c) design bug, not the
  agent: (1) **sampling** — `--limit 12` over a category-ordered `tasks.json` (tool_use
  first) ran ZERO planning/self_correction tasks, so `p_plan`/`p_correct` are `pass/0 = 0`
  by construction (`hiro.rs:362-386`); (2) **the judge measures trace-shape, not
  correctness** — default `category_trace` passes on `react_success` + a tool call, never
  checks the answer; tasks carry no `expected`/ground-truth at all. So `p_tool=0.333` =
  "finished with a tool call" (8/12 died at max-steps), NOT answer-correct. True
  planning/self-correction ability is UNKNOWN, not 0.
- [x] **M0.2a Stratified sampling + hybrid judge infra.** Replace `tasks.truncate(limit)`
  with `stratified_sample` (round-robin across categories — every category now runs).
  Add `expected` (deterministic) + `success_criteria` (LLM-judge) to `HiroTask`; new hybrid
  `judge_answer`: assert → LLM-judge → trace-fallback flagged `UNVERIFIED`. `passed` now
  means real correctness when ground truth exists.
  **Done-when:** compiles + unit tests (stratified balance 3/3/3 not 9/0/0; ExpectedSpec
  pass/fail). **Result:** 267 tests pass; `--hiro-smoke` validates the 60-task file. **Hybrid
  judge design chosen by Abrar.**
- [◐] **M0.2b STILL OPEN — LLM-judge deemed untrustworthy; pivoting to deterministic.**
  Two calibration runs proved a qwen3:8b LLM-judge is unstable in BOTH directions: harsh
  prompt → false negatives (80% agreement), lenient prompt → false positives crediting
  wrong/hallucinated answers (60% agreement, inflated pass@3 to a *mirage* 0.733). See
  `docs/research/eval-trust.md`. **Decision:** the trustworthy scoreboard becomes the
  **M1 repo-fix benchmark (deterministic test pass/fail)**, which a lenient judge can't
  inflate; HIRO LLM-judged tasks become a non-gating diagnostic. Tighten deterministic
  specs (sc_002/tu_005 were brittle). **M2.1 didn't fire** (synthesizes only from
  successful obs; thrash tasks have none) — real wall is failed-action recovery, not
  post-hoc synthesis.
- [◐] **M0.2b (original) Author ground truth + calibrate.** Authored a balanced calibration set —
  5 tool_use + 5 planning + 5 self_correction (15 tasks) in `hiro/tasks.json` (deterministic
  `expected` for the crisp self-correction fallbacks, `success_criteria` for env-dependent ones).
  **REMAINING:** run a balanced HIRO round (needs the local model, ~30–60 min), hand-label
  ~15 trajectories, verify the new judge agrees with human ≥ 90% and all three category metrics
  are non-degenerate. Record in `docs/research/eval-trust.md`.
  **Blocked by:** M0.2a (done).

## M1 — Wire a real benchmark
- [ ] **M1.1 Adopt a small REAL coding benchmark** runnable offline on the 3060: a curated
  mini-SWE-bench / SWE-bench-Verified-Lite subset, or a handful of real GitHub-issue tasks
  on a sample repo. Reference: `_refs/harnesses/SWE-agent`, augment-swebench-agent.
  **Done-when:** an honest `pass@1` baseline is recorded (even if near-zero).
  **Blocked by:** M0.2.

## M2 — Make the core loop actually finish (the capability grind)
- [◐] **M2.1 Drive reliability** using the built edit stack + failure taxonomy + remaining
  loop fixes (thrash/forfeit, tool/backend stability). Relentless re-measure.
  **Done-when:** toy HIRO-null `pass@3 ≥ 0.8` with a *meaningful* `p_correct`; real
  benchmark first non-zero then climbing run-over-run.
  **Blocked by:** M1.1.
  **Progress:** the calibration taxonomy showed the dominant failure (9/15) is
  **action-loop thrash** ("duplicate action blocked" → forfeit), NOT bad edits. The old
  forced-synthesis just *nudged* a stuck model and forfeited. Implemented (pending
  measurement): `synthesize_final_answer` — at the synthesis checkpoint OR after 3
  duplicate-blocks, directly call the model to produce the final answer from gathered
  observations and `finish`, instead of nudging. Builds + 267 tests pass; needs an
  A/B HIRO run to confirm it lifts pass@3.
- [ ] **M2.2 Stranger task end-to-end.** A "fix this bug in this small repo" task completes,
  verified by its own tests, fully offline. **Done-when:** green tests, no network.
  **Blocked by:** M2.1.

*M3 (try-it product) and M4 (self-improvement curve) follow in `MILESTONE.md`. Do not
start them, or any Phase 1.5–5 item below, until M2's gates pass.*

---

## Phase 0 — Diagnose the wall (do this FIRST, gates everything)
- [x] **0.1 Failure taxonomy.** Parse `artifacts/trajectories/2026-06-08/trajectories.jsonl`
  + `/tmp/ab_on.log` `/tmp/ab_off.log`. Classify every max-steps / failed task into:
  (a) bad edit-match, (b) wrong/no plan, (c) tool error, (d) judge too strict, (e) ran out
  of context. Write counts to `docs/research/failure-taxonomy.md`.
  **Done-when:** we have a ranked % breakdown of failure causes from real data.
  **Blocked by:** nothing.
  **Note:** if (a) bad edit-match is NOT top-2, re-order Phase 1 vs 3 accordingly.

## Phase 0.5 — Fix the measured `p_correct=0` cause
*Inserted after 0.1 because `docs/research/failure-taxonomy.md` found bad edit-match
at 0% and answerless finish / max-step thrash as the real top blockers.*
- [x] **0.5.1 Answer-gated finish.** `finish` must include a non-empty final answer
  payload; empty `finish {}` is rejected with a structured retry observation.
  **Done-when:** future trajectories preserve answer-bearing final actions and unit
  tests reject empty finish payloads.
  **Blocked by:** 0.1.
  **Commit:** `e1bdd99`.
- [x] **0.5.2 Repeated-failure / synthesis-forfeit guard.** Detect repeated max-step
  patterns and force either synthesis from gathered observations or `fail` before
  burning all attempts.
  **Done-when:** max-step warnings drop on the same 12-task null run.
  **Blocked by:** 0.5.1.
  **Commit:** `edf6a93`.
- [x] **0.5.3 RE-MEASURE.** Re-run the 12-task HIRO null baseline and record
  `p_tool`, `p_plan`, `p_correct`, `pass@3`, and max-step count.
  **Blocked by:** 0.5.2.
  **Result:** run `f1c8a72c-d601-4591-ad44-f1b2e6310187` recorded
  `pass@3=0.333`, `p_tool=0.333`, `p_plan=0.000`, `p_correct=0.000`,
  hard max-step warnings `0`, synthesis/forfeit stops `24`.
  **Commit:** `edf6a93`.

---

## Phase 1 — The edit lever (highest expected `p_correct` gain)
*Blocked by: 0.5.3. The taxonomy found edit-match at 0% in the current sample, so
this remains high-value but no longer precedes answer/loop reliability.*
- [x] **1.1 Hash-anchored edit tool (PRIMARY).** New `src/toolbridge/hashedit.rs`. File
  reads emit `Lnn|hash| content` (2–3 char content hash/line); edit tool takes
  `(file, line-hash, new_text)`, verifies hash before writing, rejects with a re-read
  prompt on mismatch. Reference: hashline writeup + `dirac` (in master plan Part 5).
  **Done-when:** a weak local model edits a file without reproducing surrounding text;
  a stale hash is caught and never corrupts. Unit tests for match + mismatch.
  **Blocked by:** 0.1.
  **Result:** `fs.hash_read` + `fs.hash_edit` wired through tool registry,
  policy, ReAct prompt, and coding smoke. `cargo run -- --coding-smoke` recorded
  `fs.hash_edit applied: true` and final cargo test passed in
  `artifacts/coding-smoke/2026-06-10/smoke-180908.json`.
  **Commit:** `8e8df62`.
- [x] **1.2 Edit-time verification (lint/parse gate).** After any edit, run a syntax check
  (tree-sitter, or `cargo check`/`python -c` per lang). On NEW errors: reject, show
  would-be-window vs original, block re-running the identical command. Reference:
  SWE-agent `_refs/harnesses/SWE-agent/tools/windowed_edit_linting/bin/edit`.
  **Done-when:** a syntactically-broken edit never lands; model gets the structured retry.
  **Blocked by:** 1.1.
  **Result:** `fs.write`, `fs.hash_edit`, and `fs.replace` now verify candidate content
  before final write for Rust, Python, JSON, and TOML. Broken Rust/JSON edits are rejected
  with structured `edit verification failed (...)` observations and leave the original file
  intact. `cargo run -- --coding-smoke` still passes with the verifier active in
  `artifacts/coding-smoke/2026-06-10/smoke-225252.json`.
  **Commit:** `dd69aae`.
- [x] **1.3 Windowed file ACI.** New `src/toolbridge/window.rs`: `open/scroll/goto` a
  bounded, line-numbered window instead of whole-file reads. Reference: SWE-agent
  `tools/windowed`. **Done-when:** edits use stable line ranges; tokens-per-file-touch drop.
  **Blocked by:** 1.1.
  **Result:** Added `fs.window_open`, `fs.window_goto`, and `fs.window_scroll`, each returning
  bounded `L<number>|hash| content` windows. ReAct prompt now prefers windowed reads for code,
  policy treats window tools as workspace-bound reads, and coding smoke uses `fs.window_open`
  before `fs.hash_edit`. Evidence: `artifacts/coding-smoke/2026-06-10/smoke-225830.json`
  and transcript `artifacts/transcripts/2026-06-10/93667d27-248b-45a6-916c-2b32a029b133.json`.
  **Commit:** `5a2e0b1`.
- [x] **1.4 Fuzzy apply-patch fallback.** Mirror codex
  `_refs/harnesses/codex/codex-rs/apply-patch/src/{parser,seek_sequence,streaming_parser}.rs`
  (uses `similar`). New `src/toolbridge/apply_patch.rs`. Retire exact-match `fs.replace`.
  **Done-when:** diff-style edits succeed under whitespace drift; tests mirror codex's.
  **Blocked by:** 1.1.
  **Result:** Added `src/toolbridge/apply_patch.rs` with unified-diff parsing and
  normalized-whitespace hunk matching when `git apply --check` fails. `patch.apply`
  now verifies fuzzy candidates before writing, reports fuzzy hunk counts, and the ReAct
  prompt no longer exposes legacy `fs.replace` as a default edit tool. Unit tests prove
  whitespace-drift success and ambiguous-context rejection; `cargo test --bins` passed
  264 tests. Coding smoke still passes in
  `artifacts/coding-smoke/2026-06-10/smoke-230330.json`.
  **Commit:** `b29ca08`.
- [ ] **1.5 RE-MEASURE.** Re-run the 12-task A/B (or HIRO null round) with the new edit
  stack. **Done-when:** `p_correct` and edit-success-rate recorded vs the 0.000 baseline.
  **Blocked by:** 1.1–1.4.
  **⚠ DEFERRED (2026-06-10):** subsumed by **M0 + M2** above — do not run a re-measure
  until the scoreboard is trustworthy (M0), or the number is meaningless. Phases 3/2/4/5
  below are likewise deferred until M2's gates pass.

---

## Phase 3 — Don't thrash, don't poison context
*Blocked by: Phase 1 complete (1.5 measured). Phase 2 is intentionally deferred — see below.*
- [ ] **3.1 Loop / repeated-failure detector.** Detect N identical or N consecutive-failing
  tool calls → force a strategy change or forfeit. Reference: goose
  `_refs/harnesses/goose/crates/goose/src/tool_monitor.rs` + `tool_inspection.rs`;
  SWE-agent `tools/forfeit`. **Done-when:** "max steps (20) reached" rate drops sharply
  vs current logs. **Blocked by:** 1.5.
- [ ] **3.2 Context compaction / condenser.** Summarize old turns near the window bound.
  New `src/agentd/compaction.rs`. Reference: codex `compaction`, OpenHands condenser,
  goose `context_mgmt`, `LLMLingua`. **Done-when:** long tasks stop overflowing the 8B
  window; tokens/turn bounded. **Blocked by:** 1.5.
- [ ] **3.3 RE-MEASURE** after 3.1+3.2. Record max-steps rate + `p_correct` change.
  **Blocked by:** 3.1, 3.2.

---

## Phase 2 — Fewer, better-bounded actions (AFTER Phase 3 re-measure)
*Deferred on purpose: the survey's "strong single-agent baseline" warns against over-
engineering the loop before the edit + context basics work. Only start if 3.3 shows
thrash/over-stepping is still a top failure.*
- [ ] **2.1 CodeAct action.** Single sandboxed `code.exec` (python/shell) composing steps
  in one turn. Reference: codex `code_mode.rs`. **Done-when:** mean steps/task drops.
- [ ] **2.2 Enforced Plan/Act.** Promote ToT into a read-only Plan phase (cannot edit) then
  an Act phase. Reference: cline `apps/vscode/src/core/task`. **Done-when:** no edit occurs
  before a plan exists.
- [ ] **2.3 Tool minimalism.** Audit the ~16 tools; gate rare ones behind `tool_search`
  (codex `tools/src/tool_search.rs`). **Done-when:** default prompt exposes ≤ ~8 tools.

---

## Phase 4 — Safety net + quality gate
*Blocked by: Phase 3.*
- [ ] **4.1 Checkpoints (shadow-git undo).** Snapshot before each edit; `/undo` reverts.
  Reference: cline `apps/vscode/src/integrations/checkpoints`. **Done-when:** any applied
  change is one command to roll back.
- [ ] **4.2 Judge-gated trajectory collection.** Gate `collect_trajectory` on the post-hoc
  LLM judge, not `Ok(true)`. **Done-when:** corpus is judge-verified (fixes distillation
  quality gap + gives an honest `p_correct`).
- [ ] **4.3 Skills-as-context injection.** Inject the matching self-authored skill into
  context per task. Reference: codex `skills`, SkillsBench (+16.2pp). **Done-when:**
  injected skills measurably lift matched tasks.

---

## Phase 5 — Hardening (lowest urgency)
*Blocked by: Phase 4. Do not start early.*
- [ ] **5.1 OS sandbox** behind policyd — codex `linux-sandbox`/seccomp + `bwrap`.
- [ ] **5.2 Pre-exec firewall** — codex `execpolicy` / intent-taxonomy guard (`nah`).
- [ ] **5.3 Session resume / crash recovery** — codex `rollout`.
- [ ] **5.4 Approve-before-apply channel** (agent↔UI).

---

## NOT doing (protect the thesis — do not let any agent add these)
- Multi-provider / frontier-API reach (jcode/goose have 15–30 providers). Local only.
- Boot/footprint micro-optimization. Not our bottleneck.
- Heavy multi-agent swarm orchestration. Revisit only after the 🔴 cluster lands.

---

## Status log (append one line per completed task)
- 2026-06-10: `harness-gaps` research plan and trajectory corpus merged into `main`;
  Phase 0.1 remains the next ordered task.
- 2026-06-10: Phase 0.1 taxonomy completed in `docs/research/failure-taxonomy.md`;
  bad edit-match was 0%, answerless finish/max-step thrash are the top blockers.
- 2026-06-10: Phase 0.5.1 answer-gated finish implemented in `e1bdd99`.
- 2026-06-10: Phase 0.5.2 synthesis/forfeit guard implemented and 0.5.3
  null re-measure recorded in `edf6a93`; max-step warnings dropped to 0, but
  `p_correct` remains 0.000.
- 2026-06-10: Phase 1.1 hash-anchored edit tool implemented in `8e8df62`;
  coding smoke uses `fs.hash_read` + `fs.hash_edit` and verifies final tests pass.
- 2026-06-10: Phase 1.2 edit-time verification implemented in `dd69aae`;
  broken Rust/JSON edits are rejected before final write and the hash-edit coding smoke
  still passes with verifier evidence in `artifacts/coding-smoke/2026-06-10/smoke-225252.json`.
- 2026-06-10: Phase 1.3 windowed file ACI implemented in `5a2e0b1`;
  coding smoke now reads `src/lib.rs` through `fs.window_open` and then applies `fs.hash_edit`,
  with evidence in `artifacts/coding-smoke/2026-06-10/smoke-225830.json`.
- 2026-06-10: Phase 1.4 fuzzy patch fallback implemented in `b29ca08`;
  `patch.apply` falls back to normalized-whitespace hunk matching when exact git apply fails,
  and `fs.replace` is removed from the active ReAct tool surface.
- 2026-06-11: M0.1 done — `docs/research/eval-trust.md`. The `p_correct=0`/`p_plan=0`
  headline is an EVAL ARTIFACT (only tool_use tasks ran under `--limit 12`; the judge
  checks trace-shape not correctness; tasks have no ground truth). The scoreboard is
  untrustworthy on both axes. Next: M0.2 — balanced sampling + a real answer-checking judge.
