# The Frankenstein Harness — Master Analysis & Rust Recreation Plan

**Date:** 2026-06-10
**Purpose:** Pull and analyze the source of every findable agent harness, catalog
every feature/capability and *how it is made efficient*, then plan to recreate the
load-bearing parts in Rust for Professor X — capability-first, local-first.

**Framing (from the [Agent Harness Survey](../../../_refs/Awesome-Agent-Harness/README.md)):**
a harness is the tuple **H = (E, T, C, S, L, V)** —
**E**xecution loop · **T**ool registry · **C**ontext manager · **S**tate store ·
**L**ifecycle hooks · e**V**aluation interface. Production-grade systems implement
all six; research prototypes implement 2–3. Our previous "Frankenstein" pass (MCP,
sub-agents, repo.map, ToT) touched T and E only — shallow. This is the full sweep.

**The one number that should drive everything:** our last A/B had `p_correct = 0.000`
in both arms. The survey's headline empirical results say this is *exactly* what
harness design fixes:

- **Pi Research:** Grok Code Fast 1 went **6.7% → 68.3%** on SWE-bench by changing
  *only the edit-tool format* — model unchanged.
- **Vercel:** removing **80% of tools** beat any model upgrade.
- **CodeAct:** code-as-action won **17/17** benchmarks at **−20% turns**.
- **SWE-agent ACI:** interface design *outweighs model capability* as the primary
  determinant of success.

So this plan tags every feature by **capability impact** — does it plausibly move
`p_correct`? — and front-loads the ones that do.

---

## Part 1 — The harnesses pulled (source in `_refs/harnesses/`)

| Harness | Lang | LOC scale | What it is best at | Source path |
|---|---|---|---|---|
| **jcode** (1jehuang) | Rust | 70 crates / huge | Production Rust harness: swarm, selfdev/hot-reload, local embeddings, 30+ providers | `_refs/harnesses/jcode` |
| **codex** (OpenAI) | Rust | ~120 crates | Sandboxing (bwrap/seccomp), execpolicy, apply_patch, skills, memories, hooks, MCP | `_refs/harnesses/codex` |
| **goose** (Block) | Rust | 10 crates | Extensions/MCP (70+), recipes, subagents, tool monitor/inspection, context_mgmt | `_refs/harnesses/goose` |
| **aider** | Python | medium | Edit formats (SEARCH/REPLACE, udiff), repomap (PageRank), multi-coder strategies | `_refs/harnesses/aider` |
| **SWE-agent** | Python | small | ACI: windowed file nav, **lint-gated edits**, search, the "interface > model" thesis | `_refs/harnesses/SWE-agent` |
| **OpenHands** | Python/TS | large | CodeAct, condenser (context compaction), runtime event-stream, microagents | `_refs/harnesses/OpenHands` |
| **cline** | TypeScript | large | Plan/Act modes, checkpoints (shadow git), focus-chain, context window mgmt | `_refs/harnesses/cline` |

**Closed-source (cataloged from the survey + docs, not clonable):** Claude Code,
Cursor, OpenClaw/PRISM (the security-hardened runtime), AIOS (OS-level scheduling,
2.1× throughput). Patterns from these are folded into the catalog below.

**Also already in `_refs/` (research/eval, not coding harnesses):** AgentEvolver,
ASI-Evolve, Hermes-Agent, clawos — self-evolution lineage, already mined.

---

## Part 2 — Master feature/capability catalog (by E,T,C,S,L,V)

Legend — Prof X status: ✅ have · ◐ partial · ✗ missing.
Impact: 🔴 likely moves `p_correct` · 🟡 quality/UX · ⚪ infra.

### E — Execution Loop

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **ReAct / observe-think-act** | All. Canonical. | ✅ `agentd/react.rs` | ⚪ |
| **CodeAct (code as the action space)** | OpenHands/codex `code_mode.rs`: one `python`/`shell` action expresses arbitrary tool composition → −20% turns, 17/17 benches. Fewer round-trips = fewer places to fail. | ✗ | 🔴 |
| **Plan/Act separation** | cline (`apps/vscode/src/core/task`): a read-only Plan mode that *cannot* edit, then an Act mode. Stops premature thrashing. | ◐ (ToT proposes, not enforced) | 🔴 |
| **`update_plan` / live todo** | codex tool `update_plan`; cline focus-chain. A persistent checklist in context keeps long tasks on-rails. | ◐ scratchpad | 🟡 |
| **Self-correction / Reflexion** | SWE-agent, aider reflection, OpenHands. Verbal RL from failure. | ✅ Reflexion + auto-repair | 🔴 |
| **Error-recovery / forfeit** | SWE-agent `tools/forfeit`; "Hell or High Water" recovery bench. Knowing when to stop beats 20-step thrash (our logs: max-steps-reached everywhere). | ◐ | 🔴 |
| **Termination on verified success** | aider/SWE-agent gate on test/lint, not self-declaration. We collect on `Ok(true)` (agent-finish), not judge-verified. | ◐ | 🔴 |
| **Subagent spawning** | goose `agents/`, codex `thread-manager`, jcode swarm. Bounded-context delegation. | ✅ `agent.delegate` | 🟡 |

### T — Tool Registry

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **Edit-format design (the #1 lever)** | aider `coders/editblock_prompts.py` SEARCH/REPLACE; codex `apply-patch` (streaming parser + `seek_sequence.rs` *fuzzy* matching via `similar` crate, tolerates whitespace/context drift). **This is the Pi 6.7→68.3% lever.** | ◐ fs.replace (exact-match only) | 🔴🔴 |
| **Edit-time verification (lint gate)** | SWE-agent `windowed_edit_linting/bin/edit`: apply → flake8 → if *new* syntax errors, **reject**, show would-be window vs original, "DO NOT re-run the same failed command." Catches broken edits before they poison the run. | ✗ | 🔴🔴 |
| **Windowed file viewing/ACI** | SWE-agent `tools/windowed`: open/scroll a file in a fixed window with line numbers, not whole-file dumps. Bounds tokens, gives stable edit coordinates. | ✗ (we read whole files) | 🔴 |
| **Tool minimalism** | Vercel: −80% tools beat a model upgrade. codex ships ~7 core tools. We have ~16 + MCP. | ✗ (we add, never prune) | 🔴 |
| **`apply_patch` as one structured tool** | codex: a single patch tool with a strict grammar + fuzzy apply, self-invoked as a subprocess. | ◐ | 🔴 |
| **Tool monitoring / inspection** | goose `tool_monitor.rs`, `tool_inspection.rs`: detect repeated-failing tool calls, loop detection, schema validation. | ◐ policyd gates, no loop-detect | 🔴 |
| **Schema-first tool contracts** | codex `tools/json_schema.rs` + policy fixtures. Validated args reduce interface misuse. | ◐ | 🟡 |
| **MCP client** | All. | ✅ `toolbridge/mcp.rs` | ⚪ |
| **agentgrep / fast code search** | jcode, SWE-agent `tools/search`, codex `file-search` (fuzzy). | ◐ shell grep | 🟡 |
| **Browser automation** | jcode, cline, SWE-agent `web_browser`. | ✗ | ⚪ |
| **Dynamic tool discovery / search** | codex `tool_search.rs`, `tool_discovery.rs`: surface only relevant tools per task (combats the 80%-tools problem). | ✗ | 🟡 |

### C — Context Manager

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **Compaction / condenser** | OpenHands condenser, codex `compaction`, goose `context_mgmt`, jcode `compaction-core`: summarize old turns when nearing the window. Local 8B has a *small* window — this is survival, not luxury. | ✗ | 🔴 |
| **Repo map (ranked)** | aider `repomap.py` (PageRank over symbol refs, tree-sitter tags). We ported a repo.map already. | ✅ `toolbridge/repo_map.rs` | 🟡 |
| **Skills-as-context injection** | codex `core-skills`/`skills`, goose `skills`, OpenHands microagents. SkillsBench: **+16.2pp**. Inject the right procedure at the right time. | ◐ self-authored tests, no injection | 🔴 |
| **Gist/hierarchical memory in-context** | ReadAgent, MemoryOS, Mem0 (90% token cut). | ◐ 5-layer memd | 🟡 |
| **Salience / "lost in the middle" handling** | Place critical facts at edges; AWM +14.9%. | ✗ | 🟡 |
| **File-ref inlining** | cline `@file`, aider `/add`. | ✅ `expand_file_refs` | ⚪ |

### S — State Store

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **Session persistence / resume** | codex `rollout` + `thread-store`, goose `session`, jcode `storage`. Replayable trajectories. | ◐ artifacts only | 🟡 |
| **Checkpoints / shadow-git undo** | cline `integrations/checkpoints`: every edit snapshotted to a shadow git → instant revert. Makes risky edits safe → agent edits more boldly. | ✗ | 🔴 |
| **Crash recovery** | OpenHands event-stream replay, codex `rollout-trace`. | ✗ | ⚪ |
| **5-layer cognitive memory** | Prof X unique (pinned/working/episodic/semantic/procedural + ICS/affect/self-model). | ✅ `memd` | ⚪ |

### L — Lifecycle Hooks

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **Sandboxing** | codex `linux-sandbox` (seccomp), `bwrap`, `windows-sandbox`; PRISM zero-fork 10-hook, <5ms, near-zero escape. SandboxEscapeBench: 15–35% escape without it. | ◐ policyd allowlist, no OS sandbox | 🟡 |
| **Exec policy / pre-exec firewall** | codex `execpolicy` + `shell-escalation`; AEGIS pre-execution firewall. Approve/deny/escalate per call. | ◐ risk-gating | 🟡 |
| **Permission prompts / approval channel** | cline, goose `permission`, codex approvals. Agent↔UI approve-before-apply. | ✗ (we apply then show diff) | 🟡 |
| **Audit log** | Prof X Merkle-chained audit (unique-ish). | ✅ `policyd` | ⚪ |
| **Hooks (pre/post tool, lifecycle)** | codex `hooks`, goose `hooks`. User-extensible interception. | ✗ | ⚪ |
| **Telemetry / token accounting** | goose `token_counter.rs`, all `usage-types`. Budget awareness mid-task. | ◐ | 🟡 |

### V — Evaluation Interface

| Capability | Who & how it's efficient | Prof X | Impact |
|---|---|---|---|
| **Trajectory capture** | All. SWE-agent `.traj`, codex rollout. | ✅ trajectories.jsonl | ⚪ |
| **Judge-gated success** | aider runs tests; SWE-agent `review_on_submit`. | ◐ LLM judge exists, not gating collection | 🔴 |
| **Self-authored tests / curriculum** | Prof X unique (self_authored_tests). | ✅ | ⚪ |
| **Self-distillation (weight-level)** | Prof X unique — no harness here trains the model. | ✅ flywheel | ⚪ |
| **Consciousness instruments (φ/PCI/LZc/meta-d′)** | Prof X unique. | ✅ | ⚪ |
| **Inspect/evidence bundle** | jcode visual-debug; Prof X `--inspect`. | ✅ | 🟡 |

---

## Part 3 — The honest read

We already match the leading harnesses on the **structural** axes (E, T-registry,
MCP, subagents, memory, S, audit) and *exceed* them on V and self-improvement. What
we are missing clusters almost entirely in the 🔴 column, and it is the same cluster
the survey says determines `p_correct`:

1. **Edit format + fuzzy apply** (aider SEARCH/REPLACE, codex apply-patch). We only
   have exact-match `fs.replace` — brittle, the model fails the match and thrashes.
2. **Edit-time verification** (SWE-agent lint gate). We apply blindly.
3. **Windowed ACI** instead of whole-file dumps.
4. **CodeAct + Plan/Act enforcement** — fewer, better-bounded actions.
5. **Loop/repeated-failure detection** (goose tool_monitor) — our logs are wall-to-wall
   "max steps (20) reached."
6. **Context compaction** — mandatory for a small local window.
7. **Checkpoints** — so bold edits are safe.

None of these are new model capability. They are harness mechanics that convert the
same 8B's outputs into completed tasks. **This is why the Frankenstein job wasn't
"done well" yet — we built the impressive-sounding parts (ToT, consciousness) and
skipped the unglamorous parts that actually finish tasks.**

---

## Part 4 — Rust recreation plan (capability-first, phased)

Each item: source to mirror → Prof X target file → done-when.

### Phase 0 — Instrument the wall (1 sitting)
- **Failure taxonomy on the existing trajectories.** Parse the 06-08 `trajectories.jsonl`
  + A/B logs: of the max-steps failures, how many are (a) bad edit-match, (b) wrong
  plan, (c) tool error, (d) judge-strictness? → `professor-x/docs/research/failure-taxonomy.md`.
  *Done-when:* we know which 🔴 to build first from data, not guess.

### Phase 1 — The edit lever (highest expected `p_correct` gain)
> **Revised after web research (2026-06-10).** Edit failure is a *mechanical interface*
> problem, not a model one — it won't be fixed by a better model, and the *weakest*
> models suffer most (our exact regime). Hard data: codex `apply_patch` = **50.7%**
> failure on Grok 4; Claude's `str_replace` needs exact-whitespace reproduction;
> Cursor trained a **dedicated 70B** just to apply edits. The converging fix is
> **hash-anchored / "hashline" edits** (also seen in `dirac`, `zerolang`): on read,
> tag each line with a 2–3 char content hash; the model edits by referencing the
> *hash* + new text, never reproducing surrounding text; validate the hash before
> applying. Reported: **−61%** output tokens (fewer retry loops), matches/beats
> str_replace, **biggest gains on the weakest models.** → For Prof X this is THE
> highest-leverage build.
- **Hash-anchored edit tool (primary).** New `toolbridge/hashedit.rs`: file reads emit
  `Lnn|hash| content`; the edit tool takes `(file, line-hash, new_text)`, verifies the
  hash still matches before writing, rejects with a re-read prompt on mismatch. *Done-when:*
  a weak local model can edit a file without reproducing surrounding text; mismatch is
  caught, never corrupts.
- **Fuzzy SEARCH/REPLACE + apply-patch as fallback.** Also mirror codex
  `apply-patch/src/{parser,seek_sequence,streaming_parser}.rs` (Rust, uses `similar` — drops
  straight in) and aider `editblock` grammar, for models/cases that prefer diffs. New
  `toolbridge/apply_patch.rs`; retire exact-match `fs.replace`. *Done-when:* edits succeed
  under whitespace/context drift; unit tests mirror codex `apply-patch/tests`.
- **Lint/parse-gated edits.** Mirror SWE-agent `windowed_edit_linting/bin/edit`: after an edit, run a syntax check (tree-sitter or `<lang> -c`/`cargo check` for Rust); on *new* errors, reject, show would-be vs original, block re-running the identical command. Wire into `react.rs` edit path. *Done-when:* a syntactically-broken edit never lands; the model gets the structured retry message.
- **Windowed file ACI.** New `toolbridge/window.rs`: `open/scroll/goto` with line numbers + a bounded window, instead of whole-file reads. *Done-when:* edits reference stable line ranges; token use per file-touch drops.

### Phase 2 — Fewer, better-bounded actions
- **CodeAct action.** A single `code.exec` tool (sandboxed python/shell) that composes
  steps in one turn; mirror codex `code_mode.rs`. *Done-when:* multi-tool tasks finish in
  fewer steps (track mean steps/task).
- **Plan/Act enforcement.** Promote ToT into a real read-only Plan phase that cannot
  edit, then an Act phase; mirror cline task modes. *Done-when:* no edits occur before a plan exists.
- **Tool minimalism + dynamic surfacing.** Audit the ~16 tools; gate rarely-useful ones
  behind `tool_search` (codex `tool_search.rs`). *Done-when:* default prompt exposes ≤ ~8 tools.

### Phase 3 — Don't thrash, don't poison context
- **Loop / repeated-failure detector.** Mirror goose `tool_monitor.rs`/`tool_inspection.rs`:
  detect N identical or N consecutive-failing calls → force a strategy change or forfeit
  (SWE-agent `forfeit`). *Done-when:* "max steps reached" rate drops sharply.
- **Context compaction / condenser.** Mirror OpenHands condenser + codex `compaction`:
  summarize old turns near the window bound. Critical for 8B. New `agentd/compaction.rs`.
  *Done-when:* long tasks stop blowing the context window.

### Phase 4 — Make boldness safe & gate quality
- **Checkpoints (shadow-git).** Mirror cline `integrations/checkpoints`: snapshot before
  each edit, `/undo` to revert. *Done-when:* any applied change is one command to roll back.
- **Judge-gated trajectory collection.** Gate `collect_trajectory` on the post-hoc LLM
  judge, not `Ok(true)`. *Done-when:* the corpus is judge-verified — fixes the distillation
  quality gap *and* gives an honest `p_correct`.
- **Skills-as-context injection.** Inject the matching self-authored skill/procedure into
  context per task (codex `skills`, SkillsBench +16.2pp). *Done-when:* retrieval-injected skills measurably lift matched tasks.

### Phase 5 — Hardening (lower urgency, keeps the research safe)
- OS sandbox (codex `linux-sandbox`/seccomp + `bwrap`) behind policyd; pre-exec firewall
  (`execpolicy`/AEGIS); session resume/crash recovery (codex `rollout`); approve-before-apply channel.

### Explicitly NOT copying (protect the thesis)
- Multi-provider/frontier-API reach (jcode/goose 15–30 providers) — violates local-first.
- Pure boot/footprint micro-optimization — real, but not our bottleneck (capability is).
- Heavy multi-agent swarm orchestration — the survey's own "strong single-agent baseline"
  paper says it rarely beats one good agent; revisit only after the 🔴 cluster lands.

---

---

## Part 5 — External resources (web research, 2026-06-10)

"Harness engineering" is now a named discipline (the 2026 successor to prompt/context
engineering — *Agent = Model + Harness*). The field's own framing matches ours: models
have commoditized into a narrow band; the harness decides production success. One stat
worth pinning: with a governed context layer, structured-task accuracy goes from
**10–31% → 94–99%** — and that gap is *largest for small models*, i.e. our thesis.

**Curated lists / leaderboards (start here):**
- `ai-boost/awesome-harness-engineering` — tools/patterns/evals/memory/MCP/permissions/observability.
- `Picrew/awesome-agent-harness` — 279 entries, implementation-first, verified 2026-06-10.
- `RyanAlberts/best-of-Agent-Harnesses` — 100+ harnesses ranked, scored weekly.
- `YennNing/Awesome-Code-as-Agent-Harness-Papers` — the CodeAct/code-as-harness paper line.
- `walkinglabs/awesome-harness-engineering`, `AutoJunjie/awesome-agent-harness`.

**Directly relevant to our build:**
- **Edit tools:** `dirac` (hash-anchored edits + AST manipulation), `zerolang` (graph-first
  edits via compiler ProgramGraph, no text patches), Cursor's dedicated apply-model, the
  "hashline" writeup. → informs Phase 1.
- **AutoHarness** (arXiv 2603.03329, Google DeepMind) — synthesize a code harness that makes
  a *small* model beat a larger one (Gemini-2.5-Flash > 2.5-Pro by eliminating illegal
  actions). There is a **Rust impl** (`gyc567/AutoHarness`) using tree search + Thompson
  sampling — conceptually our LCAP/UCB1 bandit applied to harness code itself. → ties to our
  evolution loop; the smaller-model-wins result is *our exact thesis* with a citation.
- **Context/compaction:** `LLMLingua` (≤20× prompt compression), `Token Savior` (−77%
  tokens via symbol pointers), `codebase-memory-mcp` (tree-sitter index, −120× tokens),
  `context-mode` (BM25-offload bulky tool output). → informs Phase 3 compaction.
- **SWE-bench references:** `augmentcode/augment-swebench-agent` (#1 open-source, small &
  readable), the Pier/Harbor eval harness (runs mini-swe-agent/claude-code/codex/opencode).
- **Permissions:** `nah` (intent-taxonomy guard, not command allowlists), Open Agent
  Passport (synchronous pre-action auth + crypto audit) — both sharper than our static
  allowlist. → informs Phase 5.
- **Other full harnesses to mine:** `HKUDS/OpenHarness` (open agent harness w/ built-in
  personal agent), `BulloRosso/etienne`, `revfactory/harness` (meta-skill that designs
  agent teams). `revfactory/claude-code-harness` measured structured pre-config = +23.8%
  (basic) → +36.2% (expert) task quality.

**Learning/reference:** "Learn Harness Engineering" (project-based course on env/state/
verification/control for Codex & Claude Code); Augment Code's harness-engineering guide;
the "Harness Problem / edit tool" writeup.

> Takeaway: the external field independently converged on the same diagnosis as Part 3 —
> the edit interface + context governance are where small-model reliability is won. The
> hashline finding is concrete enough to change Phase 1 (above). None of these repos are
> local-first weight-evolving research vehicles, so Prof X's V-layer/self-distillation
> stays unique; we're importing their E/T/C mechanics, not their identity.

---

## Sequencing rule
Do **Phase 0 → 1 → 3** before anything else. Phase 1 (edit lever) and Phase 3 (anti-thrash
+ compaction) are where the survey's evidence concentrates the `p_correct` gains. Everything
in Parts 1–2 above is grounded in the cloned source under `_refs/harnesses/` — file paths are
cited inline so each port starts from a real reference, not a description.
