# 🧭 Decisions & Principles — Professor X

The durable rules we operate by, with the *why*. New entries graduate here from devlog entries once
they've earned it. Newest first.

---

### D-007 · Train the model in the exact format you serve it in
A model fine-tuned in Qwen3's chat template looped forever in the benchmark, which drives it via
raw `/api/generate` ReAct text. Same model, different prompt shape → out-of-distribution → garbage.
**Rule:** training format must match the serving/inference format byte-for-byte in structure.
*(2026-06-17, devlog #9)*

### D-006 · A "reject" against a saturated benchmark means nothing
If the baseline already scores ~100%, the gate cannot show improvement (you'd need >100%). A reject
there is a *measurement-resolution* failure, not a model failure. **Rule:** keep the benchmark hard
enough that the baseline has real headroom; grow it before trusting reject/accept.
*(2026-06-17, devlog #10)*

### D-005 · Verify a model *halts* before spending hours measuring it
A non-halting model made one gate pass run ~12h. **Rule:** before any benchmark sweep, a pre-gate
check must confirm the model stops (`done_reason=stop`) and emits a valid action **in the format the
bench uses** — fail in seconds, not hours. *(2026-06-17, devlog #8/#9)*

### D-004 · When AI "breaks," suspect the plumbing before the model
Of ten problems in one session, nine were environment/serving/packaging; one-line config fixes, not
model fixes. **Rule:** check the harness, serving config, and formats before concluding the model is
bad. *(2026-06-17)*

### D-003 · Don't touch the ruler to make a candidate pass
Prefer fixes that change the *candidate* (the model) over fixes that change the *evaluation harness*
— changing the ruler invalidates comparisons and invites self-deception. *(2026-06-17, Option A vs B)*

### D-002 · Single-instance + free-the-GPU before training
A stray parallel run loaded a 10GB model onto the 12GB card and OOM'd training. **Rule:** `flock`
lock + unload resident models + wait for VRAM before QLoRA. *(2026-06-17, devlog #6)*

### D-001 · No-sudo, offline-first by default
Every environment fix this project needs (pip, dev headers, llama.cpp, base weights) has a no-sudo
path; downloads are cached so later runs are offline. Keeps the "$400 GPU, anyone can run it" thesis
honest. *(2026-06-17)*

---

> Older, project-level principles also live in `professor-x/docs/research/eval-trust.md`
> (trust-the-scoreboard / M0) and `professor-x/PLAN_11_10.md` (the north-star plan).
