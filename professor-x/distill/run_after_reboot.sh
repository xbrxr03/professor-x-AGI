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

# 0b. Activate an isolated venv (system python3.12 is PEP-668 externally-managed and has no pip).
#     Bootstrapped once via `python3 -m virtualenv distill/.venv`; create it here if missing so the
#     runner stays turnkey. All `python3` calls below then resolve to the venv interpreter.
if [ ! -x distill/.venv/bin/python ]; then
  echo "== creating training venv (distill/.venv) =="
  python3 -m virtualenv distill/.venv || { echo "STOP: could not create venv (need: python3 -m pip install --user virtualenv)"; exit 1; }
fi
# shellcheck disable=SC1091
source distill/.venv/bin/activate
echo "  using python: $(command -v python)  ($(python --version 2>&1))"

# 0c. Triton JIT-compiles CUDA kernels at runtime with gcc, which needs Python dev headers
#     (Python.h). System python3.12 ships none (no python3.12-dev; apt needs sudo). Bootstrap them
#     no-sudo into distill/.pydev (apt-get download + dpkg-deb -x), then expose via CPATH — gcc
#     searches CPATH even though Triton's hardcoded -I/usr/include/python3.12 is empty. The second
#     dir resolves Debian's multiarch pyconfig.h redirect.
PYDEV=distill/.pydev
if [ ! -f "$PYDEV/usr/include/python3.12/Python.h" ]; then
  echo "== bootstrapping Python dev headers (no-sudo) into $PYDEV =="
  _td=$(mktemp -d)
  ( cd "$_td" && apt-get download python3.12-dev libpython3.12-dev ) \
    && for d in "$_td"/*.deb; do dpkg-deb -x "$d" "$PWD/$PYDEV"; done \
    || echo "  WARN: header bootstrap failed; if Triton compile fails: sudo apt install python3.12-dev"
  rm -rf "$_td"
fi
export CPATH="$PWD/$PYDEV/usr/include/python3.12:$PWD/$PYDEV/usr/include${CPATH:+:$CPATH}"

# 1. One-time training deps. (rich is pulled in transitively by unsloth_zoo but not declared by it,
#    so list it explicitly — its absence crashes the trainer AFTER the ~2hr collection step.)
echo "== [1/6] installing training deps =="
python -m pip install -q unsloth "trl<0.10" peft bitsandbytes accelerate datasets rich \
  || { echo "STOP: pip install failed"; exit 1; }

# 2. Build release + generate diverse fixtures + collect TEST-VERIFIED trajectories.
# TEACHER DISTILLATION: collect with a STRONGER teacher (qwen3:14b, which fits the 3060's 12GB
# and — verified pre-reboot — solves the hard tasks the 8B fails: fix_004, fix_013). Distilling the
# teacher's *verified* solutions into the 8B student teaches the failure frontier, which a
# self-distillation pass (8B on its own passes) cannot. This is what gives the gate real headroom
# above the 0.857 baseline. Override with TEACHER_MODEL= if desired. (32B is too slow here: a
# ~19GB q4 model exceeds 12GB VRAM and falls back to CPU.)
# SKIP_COLLECT=1 resumes from training, reusing already-curated data on disk — so a downstream
# failure (e.g. a missing training dep) doesn't force re-running the ~2hr teacher collection.
if [ -n "${SKIP_COLLECT:-}" ]; then
  echo "== [2/6] SKIP_COLLECT set — reusing existing curated data, skipping collect+curate =="
  N=$(wc -l < distill/data/curated.jsonl 2>/dev/null || echo 0)
  echo "  curated trajectories on disk: $N"
  [ "$N" -ge 20 ] || { echo "STOP: SKIP_COLLECT set but curated data is thin/missing ($N)."; exit 1; }
else
echo "== [2/6] build + generate fixtures + collect TEACHER-verified trajectories =="
cargo build --release --quiet
python3 distill/gen_fixtures.py
TEACHER_MODEL="${TEACHER_MODEL:-qwen3:14b-q4_K_M}"
echo "  teacher model: $TEACHER_MODEL  (student/base for QLoRA stays qwen3:8b)"
for pass in 1 2; do            # a couple passes: the stochastic teacher covers different subsets
  echo "  teacher collection pass $pass…"
  REPO_FIX_TASKS=scripts/benchmarks/repo_fix/tasks_corpus.json \
  PROFESSOR_X_DATA_DIR="$HOME/.professor-x" ./target/release/professor-x \
    --repo-fix-bench --model "$TEACHER_MODEL" 2>/dev/null | grep "pass@1" || true
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
fi

# 4. QLoRA fine-tune (the long part). SKIP_TRAIN=1 reuses an existing merged model on disk.
echo "== [4/6] QLoRA fine-tune =="
if [ -n "${SKIP_TRAIN:-}" ] && [ -f distill/out/gguf/config.json ]; then
  echo "  SKIP_TRAIN set — reusing merged model in distill/out/gguf"
else
  python3 distill/train_qlora.py || { echo "STOP: training failed (see above)"; exit 1; }
fi

# 5. Serve the distilled model — auto-detect the produced artifact, no manual Modelfile edit.
# Conversion path (verified): Unsloth's own GGUF export wants `sudo apt install cmake
# libcurl4-openssl-dev` and dies on the prompt non-interactively; Ollama's safetensors importer
# rejects Qwen3 ("unsupported architecture Qwen3ForCausalLM"). The no-sudo route that works is
# llama.cpp's pure-Python convert_hf_to_gguf.py + a libcurl-free llama-quantize build. (One-time:
# the llama.cpp clone may need authorization; pre-place distill/llama.cpp to skip it.)
echo "== [5/6] serve distilled model =="
cd distill
MERGED_GGUF=$(ls out/gguf/*.gguf 2>/dev/null | grep -iE 'q4_k_m' | head -1)
if [ -z "$MERGED_GGUF" ] && [ -f out/gguf/model.safetensors.index.json ]; then
  echo "  no GGUF found — converting merged safetensors -> q4_K_M via llama.cpp (no-sudo)…"
  python -m pip install -q cmake gguf || true
  if [ ! -f llama.cpp/convert_hf_to_gguf.py ]; then
    git clone --depth 1 https://github.com/ggml-org/llama.cpp llama.cpp \
      || { echo "STOP: llama.cpp clone failed (authorize it, or pre-place distill/llama.cpp)"; exit 1; }
  fi
  if [ ! -x llama.cpp/build/bin/llama-quantize ]; then
    cmake -S llama.cpp -B llama.cpp/build -DGGML_CUDA=OFF -DLLAMA_CURL=OFF \
      -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=OFF >/dev/null \
      && cmake --build llama.cpp/build --target llama-quantize -j"$(nproc)" >/dev/null \
      || { echo "STOP: llama-quantize build failed"; exit 1; }
  fi
  python llama.cpp/convert_hf_to_gguf.py out/gguf --outfile out/gguf/distilled-f16.gguf --outtype f16 \
    || { echo "STOP: HF->GGUF convert failed"; exit 1; }
  llama.cpp/build/bin/llama-quantize out/gguf/distilled-f16.gguf out/gguf/distilled-Q4_K_M.gguf Q4_K_M \
    || { echo "STOP: quantize failed"; exit 1; }
  MERGED_GGUF=out/gguf/distilled-Q4_K_M.gguf
fi
if [ -n "$MERGED_GGUF" ]; then
  printf 'FROM ./%s\nPARAMETER temperature 0.3\nPARAMETER num_ctx 16384\n' "$MERGED_GGUF" > Modelfile
  ollama create professor-x-distilled -f Modelfile || { echo "STOP: ollama create failed"; exit 1; }
elif [ -f out/gguf/adapter.gguf ]; then
  printf 'FROM qwen3:8b-q4_K_M\nADAPTER ./out/gguf/adapter.gguf\nPARAMETER temperature 0.3\nPARAMETER num_ctx 16384\n' > Modelfile
  ollama create professor-x-distilled -f Modelfile || { echo "STOP: ollama create failed"; exit 1; }
else
  echo "STOP: no servable artifact in distill/out/gguf (no GGUF, no safetensors, no adapter)"; exit 1
fi
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
