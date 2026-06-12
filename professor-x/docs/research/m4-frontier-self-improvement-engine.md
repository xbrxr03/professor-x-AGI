# M4 frontier — the real self-improvement engine (design)

**Date:** 2026-06-12. The honest result that motivates this: across 3 evolution runs, the
**empirical fitness gate works** (it never accepts a non-improvement), but a **weak local 8B
cannot author the improvements** — even shown its exact failures, its prompt/skill proposals
never beat baseline. Yet the *manual* loop lifted repo-fix **0.50 → 0.85**. The difference is
the two things the autonomous loop currently can't do:

1. The wins were **CODE-level** (greedy-loop temperature escalation; forgiving hash-edit) — the
   autonomous loop only touches prompts/skills/config (code = `Middleware` = human approval).
2. The wins came from **reading the failing trajectory** and identifying the *mechanical* cause
   — diagnosis an 8B can't do well, but which is mostly *deterministic pattern-matching*.

So the real engine is **diagnose → propose a code diff → human approves → measure (gate)**, with
a proposer stronger than an 8B for the authoring step. NOT a fully-autonomous code mutator —
that's the misevolution risk the project's identity/ICS gates exist to prevent.

## Architecture

```
  ┌─ run repo-fix bench (trustworthy, ungameable) ─────────────┐
  │   collect per-failed-task trajectory + diff                │
  ▼                                                            │
  DIAGNOSE (automatable, deterministic)                        │
   classify each failure from the trajectory:                  │
     GREEDY-LOOP | NO-EDIT | EDIT-TOOL-REJECT | WRONG-EDIT |    │
     HALLUCINATION | TOOL/BACKEND                               │
   aggregate → dominant failure mode + the harness component    │
   it implicates (e.g. EDIT-TOOL-REJECT → toolbridge/hashedit). │
  ▼                                                            │
  PROPOSE a concrete CODE diff (needs a strong proposer —      │
   NOT the local 8B; a frontier model or a human). Scope it to │
   the implicated component; forbid touching identity/safety.  │
  ▼                                                            │
  HUMAN APPROVES the diff (Middleware gate — non-negotiable).  │
  ▼                                                            │
  MEASURE with the empirical gate: build + full test suite +   │
   repo-fix pass@1 (K reps); accept ONLY if > baseline + MDE. ─┘ (loop)
```

## What's already built (the substrate)
- **Trustworthy fitness signal:** `--repo-fix-bench` (deterministic, can't be inflated).
- **The gate:** `--evolve-on-repofix` / `--evolve-skill-on-repofix` — accept only measured
  improvement, K reps, MDE.
- **Diagnosis capture:** `repo_fix_measure` records per-failed-task `(id, description, made_edit)`;
  the `diagnose-from-trajectory` skill + `scripts/pull_trajectory.py` classify the failure
  signature (GREEDY-LOOP / NO-EDIT / EDIT-TOOL-REJECT) from `agent_events`.
- **Safety:** `Middleware` changes require human approval; ICS/identity gates; Merkle audit;
  reward-hacking scan; sandbox build+test before apply.

## What's left to build (the frontier, multi-session)
1. **`--diagnose`** — run the bench, aggregate trajectory failure signatures into a structured
   report (dominant mode + implicated component + suggested fix direction). Automates the
   human-diagnosis step; read-only, safe. *The achievable next increment.*
2. **A strong proposer for the code step** — wire a frontier model (or a human prompt) to author
   the diff for the implicated component, scoped + safety-constrained.
3. **Diff → sandbox → full-test → repo-fix gate → human-approve** pipeline (extend
   `verify_then_apply`, which already sandboxes/compiles, to gate on a measured benchmark delta
   instead of an LLM Analyzer's opinion — the same fix M4 made for prompts, applied to code).

## The honest claim this supports
This is *stronger* than ARIS's `/meta-optimize` and the legacy loop (both accept changes on
LLM-approval, never measuring). It is also *safe*: code never self-mutates without a human and a
measured win. The novel result isn't "the agent rewrites itself unattended" — it's **"harness
improvement is automatable up to the code-authoring + approval step, and every accepted change is
empirically proven on an ungameable benchmark."**
