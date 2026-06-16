# Distillation flywheel — what's done, and the ONE thing left (post-reboot)

## State (2026-06-13)
The flywheel is **fully built and turnkey** except for one environment blocker that only a
reboot fixes. Everything that can be automated, is.

**Done (automated):**
- ✅ **Verified-correct collection** — a repo-fix test PASS (green) now collects the agent's
  solving trajectory as gold-standard SFT data (`collect_trajectory` → `trajectories.jsonl`).
- ✅ **Diversity generator** — `distill/gen_fixtures.py` emits validated (red→green) diverse bug
  fixtures → `scripts/benchmarks/repo_fix/tasks_corpus.json` (14 curated + 18 generated = 32; add
  templates to scale further).
- ✅ **`REPO_FIX_TASKS` env** points the bench at the big corpus for collection (vs the headline 14).
- ✅ **Turnkey runner** — `distill/run_after_reboot.sh` does deps → generate → collect → curate →
  QLoRA → serve → **ICS-gate** (accept the distilled model only if it measurably beats baseline on
  the ungameable repo-fix benchmark) — fully automated.

## The ONE blocker (needs Abrar)
`nvidia-smi` fails: **"Driver/library version mismatch."** PyTorch/CUDA cannot run until the
machine is **rebooted** (Ollama tolerates the mismatch with its own runtime; PyTorch won't).

## After the reboot — the only step
```bash
cd professor-x
bash distill/run_after_reboot.sh
```
That's it. It installs deps, collects, curates, trains the QLoRA, serves it as
`professor-x-distilled`, and ICS-gates it — keeping the distilled model only if it *measurably*
lifts repo-fix pass@1 (a flywheel turn). Log: `/tmp/distill_flywheel.log`.

## Pre-reboot work already done (2026-06-16) — so the run is shorter + the before/after is honest
- ✅ **Ollama confirmed working pre-reboot** (only PyTorch/CUDA is blocked) — so trajectory
  *collection* and the *baseline measurement* don't need the reboot and were front-loaded.
- ✅ **Baseline pinned** in `distill/baseline_prereboot.txt` (qwen3:8b mean repo-fix pass@1). The
  post-reboot run cross-checks its fresh baseline against this; a large drift = something's off.
- ✅ **Corpus front-loaded** — verified trajectories collected pre-reboot into
  `artifacts/trajectories/<date>/` (curate.py globs all dates, so they're included).
- ✅ **Gate hardened against the noise-tail mirage** — step 6 now measures BOTH models as a
  **K-pass mean** (`GATE_PASSES`, default 3), not a single noisy run, and writes a durable
  before/after artifact to `artifacts/distill/<date>/before-after-*.json`. This is the discipline
  that caught the retracted M4 "rise": one measurement grazing the MDE is not a result.

## Honest caveats
- **Corpus may still be thin.** The QLoRA will run but may overfit; the gate will honestly REJECT
  if there's no measured gain. To strengthen: add templates to `gen_fixtures.py`.
- **The gate decides, not hope.** A rejected distilled model means it didn't learn enough yet —
  grow the corpus and re-run. An accepted one (mean beats baseline by > MDE) means the floor rose.
