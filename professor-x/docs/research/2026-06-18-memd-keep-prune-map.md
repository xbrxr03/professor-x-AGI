# memd/ keep · prune · wire map (2026-06-18)

Goal: decide which of the ~26 `src/memd/` modules belong in the **unified, memory-driven, dual-lever
self-improvement loop** (the product spine), which are support, which are a *separate* direction
(consciousness) to defer, and which are dead infra to wire-or-cut. Driven by the reference audit
(refs in the live agent loop `agentd/`, the evolution loop `evolved/`, and `main/serve`).

Loop the spine must support:
`work → memory of verified work → (a) harness diagnosis+fix  (b) distillation corpus → GATE (proven
gains only) → identity check → repeat`, on a consumer GPU.

| module | agent / evolve / cli | role | verdict |
|---|---|---|---|
| **task_runs** | 1 / 1 / 2 | record of every task run (the fuel) | **KEEP-CORE** |
| **causal_traces** | 1 / 0 / 1 | action→outcome traces (fuel + diagnosis) | **KEEP-CORE** |
| **episodic** | 2 / 3 / 2 | episodic memory + "sleep" consolidation | **KEEP-CORE** |
| **semantic** | 2 / 3 / 1 | consolidated facts (promotion target) | **KEEP-CORE** |
| **procedural** | 1 / 1 / 1 | learned skills → the harness lever | **KEEP-CORE (strengthen)** |
| **self_authored_tests** | 0 / 1 / 1 | agent writes its own curriculum/benchmark | **KEEP-CORE** |
| **self_model** | 1 / 1 / 1 | identity snapshot (the gate) | **KEEP-CORE** |
| **ics** | 0 / 1 / 2 | identity/boundary conservation (the gate) | **KEEP-CORE** |
| **events** | 1 / 3 / 2 | event log (audit trail / public logs) | **KEEP-CORE** |
| **transcripts** | 1 / 1 / 1 | full work transcripts (fuel) | **KEEP** |
| **working** | 1 / 2 / 2 | working memory / context mgmt | **KEEP** |
| **pinned** | 1 / 0 / 1 | pinned must-not-forget facts | **KEEP** |
| **metacognitive** | 1 / 2 / 1 | self-monitoring (diagnosis aid) | **KEEP (verify depth)** |
| **coding_sessions** | 0 / 0 / 2 | memory of coding sessions — **relevant but UNWIRED** | **WIRE** |
| **coding_smoke** | 0 / 0 / 2 | coding smoke tests | **KEEP (test util)** |
| phi | 2 / 1 / 2 | IIT integrated-information (consciousness metric) | **DEFER (consciousness)** |
| pci | 0 / 0 / 1 | perturbational complexity (consciousness metric) | **DEFER (consciousness)** |
| free_energy | 0 / 1 / 1 | free-energy principle | **DEFER (consciousness)** |
| narrative | 0 / 1 / 1 | narrative self | **DEFER (consciousness)** |
| computational_body | 1 / 0 / 0 | interoception (`<body>` conserve/explore hint) | **DEFER (keep prompt hook only)** |
| affect | 1 / 0 / 1 | valence (`<affect>` hint) | **DEFER (keep prompt hook only)** |
| self_prediction | 1 / 0 / 1 | predictive self-model / metacog | **DEFER (consciousness-adjacent)** |
| autonomy_queue | 0 / 0 / 2 | autonomous-ops task queue (unwired) | **DECIDE: wire for M4-autonomy or cut** |
| autonomy_health | 0 / 0 / 1 | autonomous-ops health (unwired) | **DECIDE: wire for M4-autonomy or cut** |
| work_loops | 0 / 0 / 2 | work-loop gate records (unwired) | **DECIDE: wire or cut** |

Identity/gate note: the **behavioral fingerprint** lives in `src/evolved/bf.rs` (not `memd/`) — it's
part of the identity-preservation gate alongside `self_model` + `ics`; keep.

## Summary
- **KEEP-CORE (10):** task_runs, causal_traces, episodic, semantic, procedural, self_authored_tests,
  self_model, ics, events — plus transcripts. These ARE the spine; most are already wired in either
  the live or evolution loop. Work = connect them into ONE loop, not rebuild.
- **KEEP support (5):** working, pinned, metacognitive, coding_smoke, + **WIRE coding_sessions**.
- **DEFER — consciousness direction (7):** phi, pci, free_energy, narrative, computational_body,
  affect, self_prediction. These are Abrar's separate consciousness bet (see invention_direction /
  consciousness_seeds), NOT the coding-agent product. Leave in the repo, take them OUT of the product
  self-improvement loop, invest nothing now. (Keep the lightweight `<body>`/`<affect>` prompt hints
  if they measurably help; otherwise trim.)
- **DECIDE (3):** autonomy_queue, autonomy_health, work_loops — autonomous-operation infra, currently
  dead. Keep only if/when we build the unattended M4 loop; otherwise cut as breadth.

## Implication
The spine exists and is mostly wired — the job is **unify (connect the 10 core + wire coding_sessions
into one loop) + prune (defer the 7 consciousness modules, resolve the 3 autonomy ones) + prove (on a
headroom benchmark)**. No new memory modules needed. Next: design the headroom benchmark, then wire
the unified loop around these KEEP-CORE modules.
