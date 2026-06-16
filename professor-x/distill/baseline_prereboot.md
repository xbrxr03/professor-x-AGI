# Pinned pre-reboot baseline (qwen3:8b-q4_K_M)

- benchmark: repo-fix (14-task gate set, `scripts/benchmarks/repo_fix/tasks.json`)
- measured: 2026-06-16, pre-reboot (Ollama; PyTorch/CUDA still blocked)
- **pass@1 = 0.857 (12/14)** — single pass; the post-reboot gate computes a K-pass mean.
- failed tasks: fix_004, fix_013 (agent edited but didn't make the test green).

The post-reboot `run_after_reboot.sh` cross-checks its fresh baseline mean against this number;
a large drift signals an environment/harness problem, not a real model change.
