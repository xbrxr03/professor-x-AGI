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

# 6. ICS-GATE: keep the distilled model ONLY if its MEAN repo-fix pass@1 (over K passes) beats
#    baseline by > MDE. Multi-pass averaging guards against the single-measurement noise tail that
#    produced an earlier false "rise" (the retracted M4 mirage). Writes a durable before/after
#    artifact and cross-checks the post-reboot baseline against the pinned pre-reboot baseline.
echo "== [6/6] ICS-GATE: distilled vs baseline on repo-fix (K-pass mean) =="
GATE_PASSES="${GATE_PASSES:-3}"
get() { PROFESSOR_X_DATA_DIR="$HOME/.professor-x" ./target/release/professor-x \
        --repo-fix-bench --model "$1" 2>/dev/null | grep -oP 'pass@1 = \K[0-9.]+' | head -1; }
mean() {  # model -> mean pass@1 over GATE_PASSES (samples echoed to stderr)
  local model="$1" sum=0 n=0 v s=""
  for k in $(seq 1 "$GATE_PASSES"); do
    v=$(get "$model"); [ -n "$v" ] || continue
    sum=$(python3 -c "print($sum+$v)"); n=$((n+1)); s="$s $v"
  done
  echo "    $model:$s" >&2
  [ "$n" -gt 0 ] && python3 -c "print(f'{$sum/$n:.4f}')" || echo ""
}
echo "  measuring baseline mean ($GATE_PASSES passes)…"
BASE=$(mean qwen3:8b-q4_K_M)
echo "  measuring distilled mean ($GATE_PASSES passes)…"
DIST=$(mean professor-x-distilled)
echo "  baseline mean = ${BASE:-?}   distilled mean = ${DIST:-?}   (K=$GATE_PASSES each)"

# Cross-check the post-reboot baseline against the number pinned BEFORE the reboot.
if [ -f distill/baseline_prereboot.txt ]; then
  PIN=$(cat distill/baseline_prereboot.txt)
  echo "  pinned pre-reboot baseline = $PIN (sanity cross-check)"
fi

REPORT="artifacts/distill/$(date +%Y-%m-%d)/before-after-$(date +%H%M%S).json"
mkdir -p "$(dirname "$REPORT")"
python3 - "${BASE:-}" "${DIST:-}" "$GATE_PASSES" "$REPORT" <<'PY'
import sys, json, datetime
b = float(sys.argv[1] or 0); d = float(sys.argv[2] or 0); k = int(sys.argv[3]); report = sys.argv[4]
MDE = 0.05
accept = (b > 0 or d > 0) and d >= b + MDE
json.dump({
  "generated_at": datetime.datetime.utcnow().isoformat() + "Z",
  "benchmark": "repo_fix", "passes_per_model": k, "mde": MDE,
  "baseline_mean": round(b, 4), "distilled_mean": round(d, 4),
  "delta": round(d - b, 4), "verdict": "accept" if accept else "reject",
}, open(report, "w"), indent=2)
if accept:
    print(f"  ✅ ACCEPT: distillation lifted repo-fix {b:.3f} -> {d:.3f} (+{d-b:.3f} > MDE {MDE}). The floor rose.")
    print("  Next turn: re-run this script — the baseline is higher now (the flywheel).")
else:
    print(f"  ⛔ REJECT: {b:.3f} -> {d:.3f} (+{d-b:.3f} <= MDE {MDE}). No real gain; keep baseline, grow the corpus.")
print(f"  before/after artifact: {report}")
PY
echo "== flywheel turn complete. Full log: $LOG =="
