# Parallel game plan — Claude × Codex (2026-06-23)

Trunk = `prereboot-flywheel-prep`. Every phase ends in a measured, honestly-reported gate
(verify-the-ruler). Nothing claimed without a number. Two agents run file-disjoint in parallel;
coordination is git + `AGENTS.md` + the auto-relay below.

## Thesis & the one milestone
**Thesis:** a weak local model ($400 GPU) can self-improve on real coding work, and the moat is that
the improvement is **trustable** — a rename-invariant verifier-CODE + a Goodhart-proof, collateralized
acceptance gate that mathematically refuses gains that don't generalize. Capability is dual-lever
(on-policy distill + harness); **trust is the headline.**
**The milestone everything serves:** the first self-modification (a distilled model OR a harness/skill
change) that **CLEARS the Collateralized-TGC gate** — beats stock on held-out renamed anchors by ≥MDE,
with no per-anchor drawdown. That turns "we have a trustworthy judge" into "we have trustworthy
self-improvement."

## Division of labor (file-disjoint)
- **Claude** owns: `src/` (Rust harness), the gate (`scripts/benchmarks/repo_fix/tgc_gate.py`), the
  **Living Verifier / verifier-as-code** invention, measurement + integration, the benchmark fixtures.
- **Codex** owns: `distill/` (Python training pipeline), data collection, and the **AGI/consciousness
  theory** long-arc (DCT/CGW/CLT). Only `AGENTS.md` + `RELAY.md` are shared (append-only / structured).
- **GPU is single-owner.** One bench/train at a time. Serialized via the relay GPU lock.

## Phases, milestones, tasks

### Phase A — Consolidate + first on-policy gate (NOW → 1–2 days)
- **A1 (done):** PR #25 (Claude work) + PR #26 (Codex research/skills) → trunk. PR #24 superseded.
- **A2 [Claude, GPU, in-flight]:** finish on-policy p4 distill (stock's own verified raw-ReAct passes)
  → `made_edit%` check → Collateralized-TGC gate. Report accept/reject honestly.
- **A3 [Claude]:** if made_edit% recovers but held-out flat → hand Codex the "add teacher-frontier mix".
- **Milestone A:** a measured p4 verdict on the trust gate (first on-policy candidate).

### Phase B — Living Verifier to a real win (the invention moat) [Claude, CPU-first]
- **B1:** generalize the auto-minter — synthesize collisions across all 7 families; measure
  auto-resolve rate + probes-per-resolve (rateless-efficiency curve). KILL if it can't separate most.
- **B2:** clean open-set test (fix the beachhead 35% confound) via prototype-learning distance-rejection
  (the industrial open-set-fault-diagnosis methods). Detect a genuinely-novel fault as OOD.
- **B3:** wire detect-collision → auto-mint → append-check into the live verifier (Rust/harness).
- **Milestone B:** Living Verifier demonstrated as a mechanism (auto-codebook-growth + open-set verdict).

### Phase C — Make distillation clear the gate (capability) [Codex train / Claude gate]
- **C1 [Codex]:** on-policy + teacher-frontier mix (corrected recipe), stronger teacher (qwen2.5-coder:32b);
  serve `profx-distilled-pN`. Track `made_edit%` as the leading indicator (must climb toward 98%).
- **C2 [Claude]:** gate each candidate (Collateralized-TGC). Grow anchors 14→~35 (strengthens MDE +
  makes collateral's per-task value testable).
- **Milestone C:** first distilled model that **ACCEPTS** on the trust gate.

### Phase D — Channel-code co-design loop (big-swing synthesis) [Claude]
- **D1:** the loop — agent improves (channel noise ↓) while verifier grows (codebook tracks drift);
  measure residual decode-error → 0 under the trust gate.
- **Milestone D:** a measured co-evolution curve.

### Phase E — AGI/consciousness long-arc (parallel, parked-but-alive) [Codex]
- **E1:** pressure-test DCT against synergy / causal-emergence / active-inference overlaps; kill or sharpen.
- **E2:** turn ICS into a measurable diachronic constraint (persistence, leverage, hysteresis) — cheap kill-tests.
- **Milestone E:** DCT honestly killed or sharpened into a falsifiable measurement.

## AUTO-RELAY — keep both agents working in parallel without manual hand-off
**Problem:** today the human relays "done" between agents. Goal: completion triggers fire automatically.

**Mechanism (git-native, no new services):**
1. **`RELAY.md` (shared, structured task board)** at repo root. Each task:
   `- [ ] @owner  id  | depends: <checkable-condition> | on-done: <trigger for the other agent>`.
   Conditions are *checkable facts* (no chat): `model-served:<name>` (ollama list), `pr-merged:#N`,
   `file-committed:<path>`, `gpu-free`, `bench-result:<artifact>`.
2. **Completion protocol:** on finishing a task an agent (a) commits/pushes the artifact, (b) flips its
   `RELAY.md` line to `[x]` + writes the `on-done` trigger for the other agent, (c) pushes.
3. **Per-agent watcher (the relay daemon):** a background loop — `fetch → read RELAY.md → for each of MY
   pending tasks, eval depends: → if satisfied AND (gpu-free if a GPU task) → start it`.
   - **Claude:** the watcher is a background `until <condition>; do git fetch; sleep 270; done`
     waiter (cache-warm) that re-invokes me when a trigger fires — the same background-job-completion
     pattern already in use, generalized to poll `RELAY.md`. For longer idle, ScheduleWakeup (1200s+).
   - **Codex:** its own equivalent watcher (skill-driven continuation per its restart-handoff).
4. **GPU lock:** `RELAY.md` carries `GPU_LOCK: <free|claude|codex>`. A GPU task only starts when free;
   the agent sets the lock before train/bench and clears it after (extends the existing
   `/tmp/px_flywheel.lock`). This is what keeps the two from colliding on the 3060.
5. **Heartbeat:** each watcher appends a `RELAY.md` heartbeat line so a stalled agent is visible.

**Build (proposed, Claude-owned):** `scripts/relay.py` (read/update RELAY.md, eval conditions, claim/
release GPU lock) + a `RELAY.md` template + a `scripts/relay_watch.sh` watcher. ~1 evening. Then both
agents drive off `RELAY.md` instead of human relay. *Needs Codex to adopt the same watcher — coordinate
via AGENTS.md before building.*

## Now-state
- In-flight: on-policy p4 collection (GPU). Pending merge: #25, #26. Codex: parked on DCT + owns C1.
- First trigger to wire: `model-served:profx-distilled-p4` → Claude runs the gate (A2).
