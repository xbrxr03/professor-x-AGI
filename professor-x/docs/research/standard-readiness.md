# Standard-readiness — honest assessment against H = (E, T, C, S, L, V)

The field's own framework (Agent Harness Survey, `_refs/Awesome-Agent-Harness`) scores a harness
on six components. Here is where Professor X genuinely *leads*, and where it is honestly
*incomplete* — the gap between "a strong research harness" and "the industry standard."

## What makes it standard-SETTING (novel, others don't have it)
1. **Empirically-gated self-improvement (V→E loop).** A harness change is accepted ONLY if it
   measurably beats baseline on an *ungameable* benchmark (repo-fix, judged by a test exit code),
   not on an LLM's approval. ARIS `/meta-optimize`, the legacy loop, codex, jcode — none measure.
   *This is the principle worth standardizing: never accept an unmeasured harness change.*
2. **Trustworthy-eval discipline.** Mechanism-check the ruler before believing a number; two
   "mirages" caught this session. Most harnesses report numbers they never validated the
   instrument for (the survey's "evaluation validity crisis").
3. **The thesis demonstrated on a local 8B:** 0.50 → 0.85 on the trustworthy benchmark, purely
   from trajectory-diagnosed harness fixes — "the harness is the intelligence," on a real number.
4. **Weight-level self-distillation + identity-safe (ICS-gated) evolution** — unique to this repo.

## H = (E, T, C, S, L, V) coverage

| Comp | State | Evidence / Gap |
|---|---|---|
| **E** Execution loop | ✅ strong | ReAct + Reflexion + synthesis/forfeit + **duplicate-loop temp-escalation** + adversarial critic + ToT + sub-agents (`agentd/react.rs`). |
| **T** Tool registry | ✅ strong | the **edit stack** (`hashedit` + line-fallback, `apply_patch` fuzzy, `window`, `editverify` lint-gate), MCP, repo-map, skills (`toolbridge/`). A genuine contribution for weak models. |
| **C** Context manager | ✅ strong | MermaidCanvas overview, bounded ReAct history compaction, LCAP `num_ctx` budget, windowed reads, repo-map. |
| **S** State store | ◐ partial | event store, `TaskRunStore`, transcripts, coding-sessions, git-backed checkpoints + `/undo`. **Gap: crash-recovery replay.** |
| **L** Lifecycle/security | ◐ partial | Merkle-chained audit, risk-gating, permissions, vault, reward-hacking scan, human-approval for code (`policyd/`), optional Bubblewrap OS sandbox for `shell.restricted` with audited policy-only fallback when the host disallows user namespaces. **Gap: stronger sandbox enforcement for all autonomous code paths.** |
| **V** Evaluation | ✅ strongest | deterministic repo-fix bench + **empirical fitness gate** + automated diagnosis + HIRO + DHE/BF/LCAP + self-authored tests + consciousness instruments. The standard-setting layer. |

## The honest roadmap to "standard-complete"
1. **M4 rising curve** (in progress) — a stronger proposer (`--proposer-model qwen3:14b`) behind
   the gate; the live demo of self-improvement. *Pending a model pull + run.*
2. **L: harden sandbox enforcement** — make host sandbox availability visible in readiness checks and extend isolation to all autonomous code/proposal test paths.
3. **S: crash-recovery replay** — resume interrupted tasks from persisted task transcripts and command artifacts.
4. **Adoption surface** — the one-command install (done) + a screencast + a third-party run.

## The honest claim
Professor X already *leads the field on the V→E axis* (empirically-gated, trustworthy
self-improvement on a local model) — the genuinely novel, standard-setting idea. It is *not yet*
complete on L/S (full sandbox enforcement, crash-recovery replay), which a production standard needs.
The path is clear and each gap is a bounded build, not a research unknown.
