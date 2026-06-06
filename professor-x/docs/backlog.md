# Professor X — Backlog

Prioritized engineering backlog. Sources noted where an item came from a specific
analysis. Status: ☐ open · ◐ in progress · ✓ done.

## From jcode gap analysis (2026-06-06-jcode-vs-professor-x-gap-analysis.md)
- ✓ **Local ONNX embeddings** (DONE, commit 9496ac0). Replace the Ollama
  `nomic-embed-text` dependency with in-process ONNX/`fastembed` vector inference.
  Removes a network/process dependency and speeds every embed (retrieve_ice,
  binding, cognition, case-based confidence). jcode runs vector inference locally
  with no external service.
- ☐ **Persistent server + hot-reload** (HIGH, larger build). Mirror jcode's
  SelfDev/hot_exec: a persistent server so the evolution loop can apply a verified
  change *live* instead of requiring an operator restart. Closes the
  evolve→apply→measure loop without manual intervention.
- ☐ **Swarm file-conflict handling** (MEDIUM). jcode's swarm-core gives agents
  shared-repo access with conflict avoidance. Prof X's sub-agents (`agent.delegate`)
  have no scope arbitration — add scope-locks so parallel sub-agents can't clobber.
- ☐ **Browser automation tool** (LOW). jcode has it; Prof X does not. Only if a
  use case demands it.

## Distillation flywheel (the untested headline thesis)
- ◐ **Fill the corpus** — BLOCKED by capability: self-authored curriculum tasks
  mostly FAIL the judge (success ~0.2), so judge-gated collection barely grows
  (stuck ~35 unique). Options: easier/graded curriculum, more volume, or accept a
  smaller corpus. The real ceiling is the agent's task success rate.
- ☐ **QLoRA fine-tune** — BLOCKED on (1) GPU driver mismatch (needs a reboot;
  Ollama tolerates it, PyTorch/CUDA won't) and (2) deps install
  (unsloth/peft/bitsandbytes/trl). After both: run `distill/train_qlora.py`,
  serve, ICS-gate (accept only if pass@3 beats baseline by >MDE AND ICS ≥ 0.70).

## Consciousness measurement program (2026-06-05/06 docs)
- ☐ **meta-d′ resolution** — the one MEASURED deficit (AUROC ~0.48). Calibration
  fixed overconfidence not resolution; a case-based-dominant tweak BACKFIRED
  (reverted). Real fix likely needs per-trial uncertainty from token logprobs
  (does Ollama expose them?), not retrieval-based signals.
- ☐ **Attention schema (AST-1)** — the clear MISSING consciousness indicator from
  the Butlin audit. Build a model of the agent's own attention/context-selection
  it can query and control.
- ☐ **Full per-step perturbational PCI** — today's was a task-level coupling
  on/off contrast (passed, n=36). The gold-standard version needs per-step module
  sampling + a direct perturbation pulse.
- ☐ **φ-rises-as-it-runs** — currently stable (homeostatic fix), not rising. May
  need the models to sharpen over a long evolution run, or a better integration
  measure than total correlation (which saturates).

## Consolidation
- ◐ **PR #10** (`harness-gaps` → `main`) — open, awaiting review/merge. Do not
  merge without explicit instruction.
