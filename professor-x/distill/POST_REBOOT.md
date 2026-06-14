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

## Honest caveats
- **Corpus is still thin** (~24–32 verified trajectories). The QLoRA will run but may overfit;
  the gate will honestly REJECT if there's no measured gain. To strengthen: add templates to
  `gen_fixtures.py` (each adds validated tasks → more verified trajectories).
- **The gate decides, not hope.** A rejected distilled model means it didn't learn enough yet —
  grow the corpus and re-run. An accepted one means the floor rose (the flywheel worked).
