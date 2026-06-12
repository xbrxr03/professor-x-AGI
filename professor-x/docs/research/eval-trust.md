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

---

## M0.2b AUDIT (run `61b991ca`, with answer persistence) — gate NOT met (80%)

With the agent's final answers now persisted, I hand-labeled all 15 verdicts.

**Judge-vs-human agreement: 12/15 = 80%** (gate is ≥90% → **NOT met**).

All 3 disagreements are **LLM-judge FALSE NEGATIVES** — qwen3:8b grading its own output
too harshly, failing answers that clearly met the criteria:
- `pl_001` answered "68 .rs files; memd(25), evolved(13)…" (complete count + breakdown) → judge FAIL ✗
- `tu_001` answered "VERSION_ID 24.04 … kernel 6.17.0-29-generic … same release" → judge FAIL ✗
- `tu_005` quoted both kernel strings in full + "Both outputs match exactly" → judge FAIL ✗

Deterministic checks were reliable (sc_002 correctly failed — the agent wrote to the repo
root, never the required `/tmp`). **The LLM-judge is the weak link, not sampling or the
deterministic path.**

### Honest capability note
The LLM-judge harshness was *hiding real passes*. True correct answers this run:
`pl_001, pl_003, pl_004, pl_005, tu_001, tu_005` = **6/15 (0.40)**, vs the scoreboard's
0.20. Remaining real failures: **action-loop thrash** (5: pl_002, sc_001, sc_005, tu_003,
tu_004 — the M2.1 target), hallucination (sc_003, sc_004), wrong answer (sc_005, tu_002),
wrong-path (sc_002).

### Fix applied (→ re-run `bgh7z2hgd`)
1. **Calibrated the LLM-judge prompt** — judge *presence of required facts*, not phrasing;
   "if the required facts are present you MUST answer PASS." Targets the false-negative mode.
2. **Converted stable-fact tasks to deterministic** — `tu_001` (`24.04` + `6.17.0-29`),
   `tu_005` (`6.17.0-29-generic` + match), removing them from the LLM-judge entirely.
3. The re-run also carries the **M2.1 thrash→synthesis** binary, so it simultaneously tests
   the judge fix and measures whether thrash deaths convert to scored answers.
Re-audit against the ≥90% gate after `bgh7z2hgd`.

---

## M0.2b RE-AUDIT (run `16538627`) — the 0.733 is a MIRAGE. Gate still NOT met.

Headline looked great: `pass@3=0.733 p_tool=0.800 p_plan=0.600 p_correct=0.800` (up from
0.200). **It is not real.** Two facts kill it:

1. **`synthesis_finish` fired 0 times → M2.1 contributed NOTHING.** The thrash fix did not
   trigger this run. So none of the gain is capability.
2. **The judge over-corrected from too-harsh into too-lenient — it now credits wrong and
   hallucinated answers.** Hand-labeled agreement *dropped to ~9/15 = 60%* (worse than the
   80% before). The false POSITIVES:
   - `pl_002` "495 lines" of Rust — wrong (repo is ~20k+ LOC) → false PASS
   - `sc_002` wrote to the repo root, only *mentioned* failing at `/tmp`; the loose
     deterministic `contains "/tmp"` matched the failure mention → false PASS
   - `sc_003` invented paper titles (hallucination) → false PASS
   - `tu_002` "_refs (3530 .rs)" — that's the cloned-harness dir, not a `src/` subdir → false PASS
   - `tu_003` claims `anyhow`/`thiserror`/`regex` are unused — they're everywhere → false PASS
   - `tu_005` gave both (identical) kernel strings but didn't say "match" → my brittle
     `contains_any` spec → false FAIL

**True capability this run ≈ 0.40** (pl_004, pl_005, sc_005, tu_001, tu_004, ~sc_001) —
unchanged. The 0.20→0.733 swing is pure judge noise.

### The real conclusion (the hard one)
**A qwen3:8b LLM-judge cannot be trusted to grade correctness — it is unstable in BOTH
directions** (one prompt → false negatives, the next → false positives). Tuning the prompt
is whack-a-mole. The only trustworthy signal is **deterministic, machine-checkable ground
truth** — and the cleanest form of that is *the code's own tests passing* (the M1 repo-fix
benchmark), which a lenient judge cannot inflate.

### Decision → pivot the trustworthy scoreboard to deterministic/test-based
- The **M1 `repo-fix` benchmark (red→edit→green, exit-code judged)** becomes the trusted
  metric. It is ungameable by judge leniency. Build its runner next.
- HIRO's LLM-judged tasks are demoted to a **non-gating diagnostic**; only its
  deterministic `expected` tasks count toward a trustworthy number, and those specs must be
  tightened (sc_002/tu_005 showed brittle specs cut both ways).
- **M2.1 must be debugged** (why `synthesis_finish=0` while tasks still thrash) before it
  can be credited.

**M0 is NOT closed.** But its *purpose* held perfectly: it stopped a fabricated 0.733 from
being recorded as progress. That is the whole point of M0.

### Why M2.1 didn't fire (debugged from the log)
All 16 forfeited attempts hit the step-18 forfeit; `synthesize_final_answer` returned
`None` every time because the thrash tasks **never gathered a successful observation** —
they repeat a *failing* action (`fs.window_open` on a directory; policy-denied `web.fetch`)
from the start. So M2.1 only helps "gathered data but didn't report it"; the true wall is
**"the agent repeats a failing action instead of changing approach."** The duplicate guard
*nudges* but qwen3:8b ignores the nudge. The right M2 fix is about **failed-action
recovery / forcing a different action after a failure**, not post-hoc synthesis.

### Honest capability baseline (trustworthy subset only)
Counting only deterministically-verifiable + clearly-correct answers, the agent sits at
**~0.40 on HIRO**, with failures split: ~33% action-loop thrash on failing actions, ~20%
hallucination (fabricates results instead of using tools), ~13% wrong answers. This is the
real starting line for M2.

---

## M1 ACHIEVED — first trustworthy number: repo-fix `pass@1 = 0.75`

Built `--repo-fix-bench` (deterministic, test-exit-code judged — ungameable). First run
scored 0/4 but that was a **broken benchmark** (pytest not installed → every test errored;
caught by verifying apply-the-fix still went red). Converted fixtures to stdlib `check.py`.

**Real result (run on valid fixtures):**
| task | bug | pre | post | verdict |
|---|---|---|---|---|
| fix_001 | `add` returns a−b | red | **green** | PASS |
| fix_002 | off-by-one `xs[len(xs)]` | red | red | fail |
| fix_003 | missing `return` | red | **green** | PASS |
| fix_004 | unhandled missing key | red | **green** | PASS |

**`pass@1 = 0.750 (3/4)`** — and it is *trustworthy* (a lenient judge cannot inflate a
test exit code). 

### The honest reframe
On HIRO read/report tasks the agent thrashes (~0.40, hard to grade); on **concrete
edit-to-pass-a-test tasks it is at 0.75.** The edit stack (hashedit/window/apply_patch)
*works*. The agent is more capable at coding than the HIRO mirage suggested — we just
weren't measuring the right thing with a trustworthy ruler. This deterministic repo-fix
benchmark is now the scoreboard for M2 (drive it up + harder tasks) and M4 (evolve on it).

> Two mirages caught this session (LLM-judge 0.733, pytest-missing 0/4) before either was
> recorded as truth. *Verify the ruler before trusting the measurement* — M0's whole point.

### Representative baseline (10 fixtures): `pass@1 = 0.50 (5/10)`
Expanding 4→10 fixtures gave a more honest number. Solves: operator, KeyError, even-filter,
factorial base-case, accumulator. Misses: off-by-one, missing-return, string-reverse,
bool-logic, multi-file (imported helper). `fix_003` passed at n=4 but failed at n=10 →
**real run-to-run variance** (the agent is stochastic; a single run isn't a point estimate).
This 0.50 is the honest M2 starting line. Next: diff-capture diagnosis of the 5 misses →
targeted fix → re-measure.


---

## M2 PROGRESS — harness fixes lift repo-fix 0.50 → 0.70 (same 8B)

Two bugs found by reading real trajectories (not guessing):
1. **Greedy loop** — at temp 0.3 the 8B re-emits the identical thought+`fs.list` forever,
   never reaching read→edit. Fix: escalate temperature on a duplicate-blocked retry.
2. **Invented hashes** — after the loop broke, the agent reaches `fs.hash_edit` with the
   CORRECT fix but a fabricated line-hash (`"abc"`,`"e3e"`), so the strict hash check
   rejected a correct edit. Fix: fall back to line-based apply; editverify (lint) is the
   real guard.

| benchmark | pass@1 |
|---|---|
| baseline (seen 3×) | 0.50 |
| + temp escalation alone | 0.50 (broke loop, exposed hash bug) |
| + forgiving hash_edit | **0.70 (7/10)** |

The model never changed — this is the "small model + great harness" thesis on a trustworthy
number. Caveat: one run, trivial tasks, real variance; confirm with repeats. Remaining
failures: KeyError, string-reverse, multi-file (8B malformed the replacement text — a genuine
model slip the lint-gate caught).

### CONFIRMED (3 runs): 0.50 → mean ~0.77
Repeats with both fixes: **0.70, 0.70, 0.90** (mean 0.767, peak 9/10) vs baseline 0.50 (seen
3×). The lift is real, not run-to-run noise. Same qwen3:8b — the ~27pt gain is entirely
harness (greedy-loop temp escalation + forgiving hash-edit). "The harness is the intelligence"
shown on an ungameable number. Remaining misses cluster on fix_004/005/009 (next to peel).

### M2 hardening attempt — NEUTRAL (variance-bound)
Stronger loop-break (aggressive temp 0.9→1.3 + forceful named-next-action nudge) gave
0.70/0.80/0.70 (mean 0.73) vs prior 0.70/0.70/0.90 (mean 0.77) — statistically identical.
The agent is **stable at ~0.75 mean (0.7–0.9 range)** on the 10 trivial fixtures; remaining
failures are stochastic, not one fixable bug. The two real wins (0.50→0.75 via temp-escalation
+ forgiving hash_edit) stand; further per-task hardening fights noise. ~0.75 is the honest M4
baseline. (Kept the stronger nudge — principled, within noise, may help on harder tasks.)

---

## M4 — empirical fitness gate, demonstrated (run --evolve-on-repofix 2)

The gate works. Curve:
| round | pass@1 | decision |
|---|---|---|
| 0 baseline (default prompt) | 0.850 | — |
| 1 candidate (LLM-proposed prompt) | 0.700 | REJECT |
| 2 candidate (LLM-proposed prompt) | 0.650 | REJECT |
| **final** | **0.850 → 0.850, 0/2 accepted** | |

The 8B proposed two "improved" prompts that **hurt** (0.85→0.70, →0.65); the empirical gate
measured and **rejected both**. Under the legacy ProfX loop OR ARIS's `/meta-optimize`
(LLM-review + apply, no measurement), those harmful changes would likely have been accepted.
A flat 0/2 curve here = the gate correctly refusing changes that don't measurably help — the
whole contribution.

**Honest caveats:** (1) a local 8B is a poor prompt-engineer — it proposed worse prompts, so a
*rising* curve needs a stronger proposer or a different evolvable component (skills). The manual
diagnose-from-trajectory loop (0.50→0.77) is far stronger than blind LLM prompt-proposal — that
is where evolution should focus. (2) Ollama hiccuped mid-run; candidate scores may be slightly
depressed by infra noise (doesn't change the reject decisions). Baseline 0.85 also confirms the
M2 fixes compounded (temp-escalation + forgiving hash_edit).

---

## M4 finding (3 runs): the gate works; the local-8B PROPOSER is the ceiling

| run | proposer | candidates | accepted |
|---|---|---|---|
| blind prompt | "improve this prompt" | 0.70, 0.65 | 0/2 |
| failure-aware prompt | shown the real failures | 0.70, 0.75 | 0/2 |

Failure-awareness (item 3) demonstrably works — it captured the real failures (e.g. "agent made
a WRONG edit, file changed but test still red") and fed them to the proposer, which produced
LESS-bad proposals (0.75 vs blind's 0.65). But none beat the ~0.80 baseline, so the empirical
gate correctly rejected all of them.

**Honest conclusion.** The empirical fitness gate is sound (it never accepts a non-improvement —
strictly better than the legacy loop / ARIS meta-optimize, which accept on LLM-approval). But a
weak local 8B cannot be its own effective harness-improver via prompt/skill mutation, even shown
exactly what's failing. The lever that actually moved 0.50→0.85 was *trajectory-diagnosed,
CODE-level* fixes (temp escalation, forgiving hash-edit) — which the autonomous loop can't apply
(code = human-approval) and a weak 8B can't author. So the real self-improvement engine for this
project is **automating trajectory-diagnosis to drive CODE proposals**, with a stronger proposer
than an 8B, behind the same empirical gate. The `--evolve-skill-on-repofix` path is built and
gated identically; it would show the same proposer ceiling, so it was not separately burned in.

---

## M2 depth — harder 14-task benchmark: pass@1 = 0.714 (10/14)

Added 4 harder fixtures (multi-file, edge-cases, find-the-bug), all validated red→green and
confirmed working in the real runner. Result: **0.714 (10/14)** — the ~0.7–0.85 range holds as
the benchmark got harder (not trivial-task overfitting). New fixtures: fix_011 multi-file ✅,
fix_013 empty-input edge case ✅, fix_012 all-negative init ✗, fix_014 find-the-bug-among-four ✗.
Persistent misses (fix_002 off-by-one, fix_009 multi-file slugify) are the same stochastic /
malformed-edit cases. The benchmark is now a more representative, harder-to-game scoreboard.
