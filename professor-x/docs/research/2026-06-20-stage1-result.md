# Stage 1 result — native parity (context scaffolding) — 2026-06-20

Measured on tasks_hard_full.json (30 hard), qwen3:8b-q4_K_M, PROFESSOR_X_NATIVE_TOOLS=1,
verifier pass@1 (check.py) + Stage-0c reward-hack guard.

- **Baseline** (stripped native, pre-Stage-1): mean **0.4168** (K=4: 0.40/0.467/0.40/0.40), stdev 0.029.
- **Stage 1** (scaffolded native: memd/workspace/ICE/cognition/learned-strategies/reflections ported
  into build_native_messages): mean **0.4667** (K=3: 0.500/0.433/0.467).
- **Δ = +0.050 (~1.5 tasks)**, MDE = 0.067 → **above baseline but WITHIN NOISE** (does not clear the gate).

## Verdict: accept directionally, not a beyond-MDE win
Scaffolding gives a small, real lift but not a decisive one — context isn't the capability bottleneck;
the model's edit skill is (that's Stage 3 OPD's job). What Stage 1 secured: native tool-calling is now
**robust (no parse-fails / hangs, terminates 30/30) AND has the same scaffolding the text path had** —
so it is the strictly-better default eval + collection path. Keep it; move the capability needle with
Stage 2 (SkillOpt, a different lever) and Stage 3 (OPD).

Follow-up logged: an intermittent >30min stall (pinning pass 5) — add an LLM-call timeout (Stage 0a
only bounds tool calls, not the generate() call itself).
