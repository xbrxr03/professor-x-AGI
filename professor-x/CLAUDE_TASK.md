# CLAUDE_TASK.md — Claude's pickup brief (parallel-code)

You are **Claude**, the `src/` (Rust) + integration/gate owner in the Claude×Codex parallel setup.
**Read `AGENTS.md` first** (the ledger: rules, checklist, log), then this. Codex owns `distill/`; you own
`src/` + the gate. **Never edit Codex's files.** You work in your own worktree off `prereboot-flywheel-prep`.

## Read first (state, in order)
- `AGENTS.md` — the live ledger (rules + per-stream checklist + append-only log).
- `docs/PROJECT_ATLAS.md` — every direction (north star → levers → inventions → parked AGI seeds).
- `docs/PLAN_PHASE3_2026-06-22.md` — the current phase (distillation flywheel + TGC trust-gate).
- `docs/research/2026-06-22-RESULT-A5-behavior-retrieval.md` — why Stream A is shelved; **bottleneck = edit-production CAPABILITY**.
- `docs/research/2026-06-21-failure-taxonomy.md` — wrong-edit-verified-fail is 60–80% of failures.

## Where things stand (2026-06-22)
- **MERGED** into `prereboot-flywheel-prep`: Stream A (behavior retrieval, flag `PROFESSOR_X_BEHAVIOR_RETRIEVAL`
  default OFF — kept as representation, use shelved), Stream B (taxonomy), Stream D (TGC gate). **Full
  suite 370/370 green.**
- **IN PROGRESS (Codex, do not touch):** Stream E on `codex/p3-distill` — QLoRA distillation
  (teacher qwen3:14b → student 8b) → serves `profx-distilled-p3`. Partial: 7 teacher trajectories collected.

## YOUR pending task: D-INTEGRATION — run the TGC trust-gate
**Trigger:** when Codex has served `profx-distilled-p3` (check `ollama list`; AGENTS.md log will note E2 done
+ "no active GPU jobs"). **Do NOT run while Codex is training — one GPU bench at a time.**
Steps:
1. Build the held-out anchor set if missing:
   `python3 -c "import json,glob; t=[x for f in sorted(glob.glob('scripts/benchmarks/repo_fix/tasks_anchor_*.json')) for x in json.load(open(f))['tasks']]; json.dump({'tasks':t},open('/tmp/tasks_anchors_all.json','w'))"`
2. Run the gate (from `professor-x/`):
   ```
   python3 scripts/benchmarks/repo_fix/tgc_gate.py \
     --baseline qwen3:8b-q4_K_M --candidate profx-distilled-p3 \
     --train scripts/benchmarks/repo_fix/tasks_families.json \
     --heldout /tmp/tasks_anchors_all.json --k 3 --gguf <p3 gguf path>
   ```
   (Sanity the logic first: `python3 scripts/benchmarks/repo_fix/tgc_gate.py --self-test`.)
3. **Report honestly (verify-the-ruler):** accept/reject, `anchor_delta`, candidate Goodhart gap.
   - **ACCEPT** (held-out renamed-anchor pass@1 +≥MDE AND gap bounded) → the distilled model GENERALIZES →
     this is Phase-3 M3.1/M3.2 (TGC trust demonstrated). Record in AGENTS.md + PROJECT_ATLAS (Lever-1).
   - **REJECT** → say so plainly; teacher/recipe insufficient at this scale → iterate (more frontier
     collection / epochs). **Never ship a model that only improves on train.** No fabricated wins.

## Rules (non-negotiable)
- **File-disjoint:** own `src/` + the gate script; never edit `distill/` (Codex's). Only `AGENTS.md` is shared (append-only).
- **GPU single-owner:** never run a bench/gate while Codex trains. Wait for the ledger's "no active GPU jobs".
- **Any `src/` change:** `cargo build --bins` + full `cargo test --bins` green BEFORE commit. Branch off
  `prereboot-flywheel-prep`. End commit messages with the Co-Authored-By line.
- **Update `AGENTS.md`:** check the box + append a log line when you finish a unit of work.

## After the gate (next, from PROJECT_ATLAS)
P2 native feature parity · P4 invention productization (Diagnostic Verifier Codes — `fault_signature.rs`
+ `signature_index.json` already exist as the substrate) · P5 real-repo benchmark (SWE-Gym/R2E). Long-arc
AGI seeds stay parked.
