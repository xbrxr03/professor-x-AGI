# M0.1 — Eval Trust Diagnosis (why p_correct & p_plan read 0)

**Date:** 2026-06-10 · **Task:** `NEXT_STEPS.md` M0.1 · **Run under analysis:**
`f1c8a72c-d601-4591-ad44-f1b2e6310187` (Phase 0.5.3 re-measure)
`pass@3=0.333  p_tool=0.333  p_plan=0.000  p_correct=0.000`.

## Verdict
**The scoreboard is untrustworthy on both axes the milestone cares about.** Neither
of the two "0.000" values means "the agent can't do it," and the one non-zero value
(`p_tool=0.333`) does **not** measure answer correctness. Concretely:

| Metric | Read | Real cause | Category |
|---|---|---|---|
| `p_correct` | 0.000 | **Never ran a self-correction task.** | sampling artifact |
| `p_plan` | 0.000 | **Never ran a planning task.** | sampling artifact |
| `p_tool` | 0.333 | "Finished with ≥1 tool call," **not** "answered correctly." | judge measures process, not correctness |

## Cause 1 — sampling artifact (kills p_plan & p_correct)
`p_plan`/`p_correct` are computed as `pass / category_total`
(`hiro.rs:384–386`), where the totals only increment for tasks of that category
actually executed (`hiro.rs:362–381`).

`hiro/tasks.json` holds **60 tasks, perfectly category-ordered**: `tu_001…tu_020`
(tool_use) **first**, then 20 planning, then 20 self_correction. The run used a flat
**`--limit 12`**, and `load_tasks()` returns them in file order — so the first 12 = **all
tool_use**. Result: `plan_total = corr_total = 0` → `div_safe(…, 0) = 0.0`.

→ `p_plan=0` and `p_correct=0` measured **nothing**. They are not evidence the agent
fails planning or self-correction; those tasks never executed. (Confirmed: the A/B logs
show only `tu_009…tu_012` and `[12/12]`; the failure-taxonomy clusters are all tool tasks.)

## Cause 2 — the judge measures trace-shape, not correctness (taints p_tool)
Default evaluator is `category_trace` (`hiro.rs:892` → `evaluate_category_trace`,
`hiro.rs:915`). It passes a task iff:
1. `react_success == true` (the ReAct loop self-declared finish), **and**
2. a category-shaped trace exists — for tool_use, ≥1 successful tool call
   (`hiro.rs:923`); for planning, a tool call or ≥2 steps; for self_correction, a
   retry/reflection/recovery signal.

**It never parses the agent's final answer, and never compares it to an expected
answer.** There is no ground truth to compare to: every task in `tasks.json` has only
`{id, category, description, difficulty}` — **no `expected`, no `evaluator`, no
assertion** (grep confirms zero such fields). Example `tu_009` asks the agent to
"verify the 'primary' model appears in the loaded models list," but nothing checks
whether it actually did. The alternative `finish_only` evaluator (`hiro.rs:903`)
literally always returns `passed: true`.

→ `p_tool=0.333` means **4/12 tool tasks finished with a tool call**; the other 8 died
at `MAX_STEPS` (`react_success=false`, rejected at `hiro.rs:893`). Even the 4 "passes"
could hold wrong answers. So `p_tool` is a *did-it-do-something* signal, not a
*did-it-get-it-right* signal.

## What this means
- The `p_correct=0.000` headline that drove the "agent can't finish anything" anxiety
  is **largely an artifact** (one category sampled; a process-only judge). The agent's
  *true* correctness on planning/self-correction is **UNKNOWN**, not zero.
- We genuinely cannot trust any number until (a) all three categories are sampled and
  (b) tasks carry ground truth and the judge checks the answer.
- Integrity note: this is exactly the kind of metric that *looks* like a result but
  isn't — the project has shipped fabricated "confirmed" metrics before; M0 exists to
  stop that.

## Hand-off to M0.2 (the fix)
1. **Balanced sampling.** Replace flat `--limit N` over a category-ordered file with
   either the full 60 or **N-per-category** sampling, so `p_tool`/`p_plan`/`p_correct`
   are all populated. (Min change: stratify in `load_tasks`/the limit path.)
2. **Real correctness judge.** Give each task ground truth + an answer check:
   - add an `expected`/`assert` field (deterministic check) or a per-task
     `success_criteria` string for an LLM-judge of the *final answer*;
   - add an `answer_check` evaluator that parses `task` output and verifies it;
   - keep `category_trace` only as a secondary "did it use tools" signal, not as
     `passed`.
3. **Re-label.** `p_correct` should mean answer-correct, not "had retry evidence."
   Either rename the trace metrics (`p_tool_trace`, …) or fold them into a separate
   diagnostic so the headline number is honest.
4. **Gate (M0.2 done-when):** on ~15 hand-labeled trajectories, the new judge agrees
   with human ≥ 90%; all three category metrics are non-degenerate on a balanced run.

---

## M0.2b Calibration run (2026-06-11, run `4ed499a6`, qwen3:8b, 15 stratified tasks)

Command: `--hiro-null 1 --hiro-limit 15 --model qwen3:8b-q4_K_M`.

### The sampling fix is validated in production
Tasks ran **interleaved across all three categories** (`tu→pl→sc→tu→…`), and the
metrics are **non-degenerate** for the first time:

| Metric | Old (0.5.3, broken) | New (4ed499a6) |
|---|---|---|
| `p_tool` | 0.333 (trace-inflated) | **0.000** (0/5) |
| `p_plan` | 0.000 (never ran) | **0.400** (2/5) |
| `p_correct` | 0.000 (never ran) | **0.200** (1/5) |
| `pass@3` | 0.333 | **0.200** (3/15) |

`p_tool` *fell* because the new judge rejects trace-only "passes" — the old 0.333 was
inflated. This is the scoreboard getting **more honest**, not the agent getting worse.

### Per-task verdicts (best of 3 attempts)
| Task | Cat | Verdict | Judge path / reason |
|---|---|---|---|
| tu_001 | tool | fail | LLM-judge FAIL |
| tu_002–005 | tool | fail | duplicate-action-blocked (thrash) |
| pl_001,002 | plan | fail | duplicate-action-blocked |
| pl_003 | plan | fail | LLM-judge FAIL (src/ not found) |
| **pl_004** | plan | **PASS** | LLM-judge (generate 3 hypotheses) |
| **pl_005** | plan | **PASS** | LLM-judge (hypotheses.md) |
| sc_001,004,005 | self_corr | fail | duplicate-action-blocked |
| sc_002 | self_corr | fail | deterministic: answer lacked `/tmp` path |
| **sc_003** | self_corr | **PASS** | LLM-judge (web-search fallback) |

### Hybrid judge: all three paths fired
deterministic (sc_002), LLM-judge (pl_003/tu_001 FAIL; pl_004/pl_005/sc_003 PASS),
and trace-fallback. Wired and live.

### Hand-label vs the ≥90% gate — PARTIAL, gate NOT yet formally cleared
- **~9/15 are structurally certain-correct**: the agent produced *no valid answer*
  (duplicate-action-blocked / no-tool-trace / ReAct-failed). A human agrees these are
  fails. Judge correct.
- **6/15 (3 passes + tu_001/pl_003/sc_002) cannot be independently verified**: this run
  persisted only an `output_hash`, not the final-answer text (no transcript, collection
  fired only conceptually). So I can't confirm the LLM-judge passes aren't false positives
  or that sc_002 isn't a false negative.
- **Verdict: do NOT claim ≥90%.** Honest status: judge is non-degenerate and plausible,
  but unaudited on the 6 decisive cases.

### Blocker found → fix before re-run
**The harness must persist the final-answer text for every task** (pass or fail), not
just a hash, or judge audits are impossible. Small change (dump `final_answer(task)` into
the attempt artifact). Then re-run and complete the hand-label.

### Capability signal (honest, for M2)
**9/15 failures were "duplicate action blocked"** — the agent repeats an identical action,
a guard blocks it, and it dies. The dominant real failure mode is **action-loop thrash**,
not bad edits. That is the concrete M2 / Phase-3 target.

