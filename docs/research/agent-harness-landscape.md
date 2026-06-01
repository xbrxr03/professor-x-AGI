# Agent Harness Landscape

Last reviewed: 2026-06-01

This file tracks the coding-agent and autonomous-agent harnesses Professor X should be measured against. The target is not only "can it edit code"; the target is an observable, auditable, rollback-capable Rust harness that can run local research cycles and improve its own harness under gates.

## Baseline Systems

| System | Interface | Core strengths to match | Gap Professor X should close |
|--------|-----------|-------------------------|------------------------------|
| [OpenAI Codex CLI](https://github.com/openai/codex) | Terminal agent | Local coding-agent loop, sandboxing, approvals, readable session transcript | Add local-first research memory, HIRO metrics, self-evolution records, and autonomous commit provenance |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview) | Terminal agent | Strong repo navigation, tool use, patching, multi-step coding workflow | Reproduce the observable work loop locally with open harness code and durable experiment artifacts |
| [Aider](https://github.com/Aider-AI/aider) | Terminal pair programmer | Git-native editing, compact UX, broad model support | Add daemon operation, safety policy, benchmark-driven evolution, and scientific audit trails |
| [OpenHands](https://github.com/All-Hands-AI/OpenHands) | Web/local dev agent platform | Browser/editor/shell environment, task sandboxing, general software-agent platform | Keep Professor X lighter, Rust-native, and focused on autonomous harness evolution rather than broad app control |
| [SWE-agent](https://github.com/SWE-agent/SWE-agent) | Benchmark/software engineering agent | SWE-bench orientation, structured repair loops, reproducible experiments | Add persistent self-model, memory, autonomous daily operation, and repo-level evolution commits |
| [Cline](https://github.com/cline/cline) | IDE/SDK/CLI agent | Tool-rich coding workflow, project rules, human approval UX | Add terminal-first observability, deterministic local policy gates, and research-run persistence |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | Terminal agent | Open terminal agent for Gemini, broad command-line workflows | Compete on local model independence, safety gates, and self-evolution measurement |
| [ClawOS](https://github.com/xbrxr03/clawos) | Local agent OS prototype | Prior local safety and tool-control instincts | Port the useful policy patterns, but make the research loop measurable and publishable |

## Professor X Harness Requirements

To be on par with modern coding-agent harnesses, Professor X needs these minimum surfaces:

- Live work view: show plan, tool calls, command output summaries, files touched, failures, retries, and artifacts.
- Replay view: reconstruct any run as a readable transcript from stored events and reports.
- Git-native operation: every accepted autonomous change maps to a commit, run id, proposal record, and verification record.
- Policy boundary: file, shell, git, network, and vault access must be explicit, auditable, and workspace-bound.
- Benchmark loop: HIRO attempts and rounds must produce task-level pass/fail data, not only narrative summaries.
- Sandbox evolution: proposed diffs run in isolated worktrees and are accepted only after tests, HIRO subset checks, reward-hacking scan, and rollback plan.
- Research artifacts: daily work, failed hypotheses, DHE traces, BF trajectories, LCAP arm state, ICS/FED records, and paper tables must be generated from stored data.

## Near-Term Build Order

1. Make the Rust CLI as observable as Codex/Claude Code for local runs: `--observe-work`, `--work-cockpit`, `--run-review`, and `--replay`.
2. Make CI and local gates unavoidable: `cargo check`, `cargo test`, policy regression tests, audit-chain tests, and replay/report shape tests.
3. Promote autonomous run artifacts into first-class GitHub-visible research output under `artifacts/`, `brain/`, and `docs/research/`.
4. Tighten evolution gates until Professor X can make safe commits without corrupting the repo or faking HIRO progress.
