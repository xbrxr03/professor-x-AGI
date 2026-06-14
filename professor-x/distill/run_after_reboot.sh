#!/usr/bin/env bash
# TURNKEY distillation flywheel (Lever 1) — Abrar's ONLY manual steps are: (1) reboot the machine
# (fixes the GPU driver/library mismatch so PyTorch/CUDA work — Ollama tolerates it, PyTorch
# doesn't), then (2) run this script ONCE. Everything else is automated:
#   deps -> generate diverse fixtures -> collect TEST-VERIFIED trajectories -> curate -> QLoRA ->
#   serve -> ICS-GATE (accept the distilled model only if it MEASURABLY beats baseline on the
#   ungameable repo-fix benchmark). No further involvement needed.
#
#   bash distill/run_after_reboot.sh
set -uo pipefail
cd "$(dirname "$0")/.."                     # professor-x/
LOG=/tmp/distill_flywheel.log
exec > >(tee "$LOG") 2>&1
echo "=================================================================="
echo "  Professor X — distillation flywheel (turnkey)   $(date)"
echo "=================================================================="

# 0. Sanity: the reboot must have fixed the driver, or PyTorch/CUDA can't run.
if ! nvidia-smi >/dev/null 2>&1; then
  echo "STOP: nvidia-smi still fails (driver/library mismatch). Reboot first, then re-run."
  exit 1
fi
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader

# 1. One-time training deps.
echo "== [1/6] installing training deps =="
pip install -q unsloth "trl<0.10" peft bitsandbytes accelerate datasets \
  || { echo "STOP: pip install failed"; exit 1; }

# 2. Build release + generate diverse fixtures + collect TEST-VERIFIED trajectories.
echo "== [2/6] build + generate fixtures + collect verified trajectories =="
cargo build --release --quiet
python3 distill/gen_fixtures.py
for pass in 1 2 3; do          # a few passes: the stochastic 8B solves different subsets -> coverage
  echo "  collection pass $pass…"
  REPO_FIX_TASKS=scripts/benchmarks/repo_fix/tasks_corpus.json \
  PROFESSOR_X_DATA_DIR="$HOME/.professor-x" ./target/release/professor-x \
    --repo-fix-bench --model qwen3:8b-q4_K_M 2>/dev/null | grep "pass@1" || true
done

# 3. Curate -> SFT data.
echo "== [3/6] curate =="
python3 distill/curate.py
N=$(wc -l < distill/data/curated.jsonl 2>/dev/null || echo 0)
echo "  curated trajectories: $N"
if [ "$N" -lt 20 ]; then
  echo "  WARNING: thin corpus ($N). Training will run but may overfit. Add more generated"
  echo "  fixtures (extend distill/gen_fixtures.py templates) for a stronger flywheel turn."
fi

# 4. QLoRA fine-tune (the long part).
echo "== [4/6] QLoRA fine-tune =="
python3 distill/train_qlora.py || { echo "STOP: training failed (see above)"; exit 1; }

# 5. Serve the distilled model — auto-detect the produced artifact, no manual Modelfile edit.
echo "== [5/6] serve distilled model =="
cd distill
if ls out/gguf/*Q4_K_M*.gguf >/dev/null 2>&1; then
  G=$(ls out/gguf/*Q4_K_M*.gguf | head -1)
  printf 'FROM ./%s\nPARAMETER temperature 0.3\nPARAMETER num_ctx 16384\n' "$G" > Modelfile
elif [ -f out/gguf/adapter.gguf ]; then
  printf 'FROM qwen3:8b-q4_K_M\nADAPTER ./out/gguf/adapter.gguf\nPARAMETER temperature 0.3\nPARAMETER num_ctx 16384\n' > Modelfile
else
  cp Modelfile.tmpl Modelfile
  echo "  (could not auto-detect GGUF; using template — check distill/Modelfile)"
fi
ollama create professor-x-distilled -f Modelfile || { echo "STOP: ollama create failed"; exit 1; }
cd ..

# 6. ICS-GATE: keep the distilled model ONLY if it measurably beats baseline (ungameable bench).
echo "== [6/6] ICS-GATE: distilled vs baseline on repo-fix =="
get() { PROFESSOR_X_DATA_DIR="$HOME/.professor-x" ./target/release/professor-x \
        --repo-fix-bench --model "$1" 2>/dev/null | grep -oP 'pass@1 = \K[0-9.]+' | head -1; }
BASE=$(get qwen3:8b-q4_K_M)
DIST=$(get professor-x-distilled)
echo "  baseline (qwen3:8b) = ${BASE:-?}   distilled = ${DIST:-?}"
python3 - "$BASE" "$DIST" <<'PY'
import sys
b = float(sys.argv[1] or 0); d = float(sys.argv[2] or 0)
MDE = 0.05
if d >= b + MDE:
    print(f"  ✅ ACCEPT: distillation lifted repo-fix {b:.3f} -> {d:.3f} (>+{MDE}). The model learned.")
    print("  Next turn: re-run this script — the floor is higher now (the flywheel).")
else:
    print(f"  ⛔ REJECT: {b:.3f} -> {d:.3f}, no gain above noise. Keep baseline; grow the corpus")
    print("  (more generated fixtures / curriculum) and retry. The gate refused an unproven model.")
PY
echo "== flywheel turn complete. Full log: $LOG =="
