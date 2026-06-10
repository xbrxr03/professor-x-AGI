# NEXT STEPS — ordered to-do (read before starting any work)

**Source of truth for the plan:** `docs/research/2026-06-10-frankenstein-harness-master-plan.md`.
This file is the *execution order*. The goal everything serves: lift `p_correct`
(last A/B = 0.000). Build the harness mechanics that turn the same local 8B's output
into FINISHED tasks — edit interface + anti-thrash + context governance first.

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

## Phase 0 — Diagnose the wall (do this FIRST, gates everything)
- [ ] **0.1 Failure taxonomy.** Parse `artifacts/trajectories/2026-06-08/trajectories.jsonl`
  + `/tmp/ab_on.log` `/tmp/ab_off.log`. Classify every max-steps / failed task into:
  (a) bad edit-match, (b) wrong/no plan, (c) tool error, (d) judge too strict, (e) ran out
  of context. Write counts to `docs/research/failure-taxonomy.md`.
  **Done-when:** we have a ranked % breakdown of failure causes from real data.
  **Blocked by:** nothing.
  **Note:** if (a) bad edit-match is NOT top-2, re-order Phase 1 vs 3 accordingly.

---

## Phase 1 — The edit lever (highest expected `p_correct` gain)
*Blocked by: 0.1*
- [ ] **1.1 Hash-anchored edit tool (PRIMARY).** New `src/toolbridge/hashedit.rs`. File
  reads emit `Lnn|hash| content` (2–3 char content hash/line); edit tool takes
  `(file, line-hash, new_text)`, verifies hash before writing, rejects with a re-read
  prompt on mismatch. Reference: hashline writeup + `dirac` (in master plan Part 5).
  **Done-when:** a weak local model edits a file without reproducing surrounding text;
  a stale hash is caught and never corrupts. Unit tests for match + mismatch.
  **Blocked by:** 0.1.
- [ ] **1.2 Edit-time verification (lint/parse gate).** After any edit, run a syntax check
  (tree-sitter, or `cargo check`/`python -c` per lang). On NEW errors: reject, show
  would-be-window vs original, block re-running the identical command. Reference:
  SWE-agent `_refs/harnesses/SWE-agent/tools/windowed_edit_linting/bin/edit`.
  **Done-when:** a syntactically-broken edit never lands; model gets the structured retry.
  **Blocked by:** 1.1.
- [ ] **1.3 Windowed file ACI.** New `src/toolbridge/window.rs`: `open/scroll/goto` a
  bounded, line-numbered window instead of whole-file reads. Reference: SWE-agent
  `tools/windowed`. **Done-when:** edits use stable line ranges; tokens-per-file-touch drop.
  **Blocked by:** 1.1.
- [ ] **1.4 Fuzzy apply-patch fallback.** Mirror codex
  `_refs/harnesses/codex/codex-rs/apply-patch/src/{parser,seek_sequence,streaming_parser}.rs`
  (uses `similar`). New `src/toolbridge/apply_patch.rs`. Retire exact-match `fs.replace`.
  **Done-when:** diff-style edits succeed under whitespace drift; tests mirror codex's.
  **Blocked by:** 1.1.
- [ ] **1.5 RE-MEASURE.** Re-run the 12-task A/B (or HIRO null round) with the new edit
  stack. **Done-when:** `p_correct` and edit-success-rate recorded vs the 0.000 baseline.
  **Blocked by:** 1.1–1.4.

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
