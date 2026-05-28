# Contributing to Professor X

Thanks for your interest in contributing! Professor X is a research project exploring self-evolving AI agent harnesses on consumer hardware. Every contribution — code, documentation, hypotheses, or bug reports — helps advance that mission.

## Getting Started

1. **Fork** the repository
2. **Clone** your fork locally
3. **Create a branch** from `main`: `git checkout -b your-feature-name`
4. **Make changes** and commit with clear messages
5. **Open a Pull Request** against `main`

## Development Setup

### Prerequisites
- Rust 1.75+ (stable)
- Ollama with `qwen3:8b-q4_k_m` model (or any compatible model)
- An NVIDIA GPU with 12GB+ VRAM (RTX 3060 or better) — or CPU-only mode for non-inference tasks
- SQLite 3.35+

### Build & Test
```bash
cd professor-x
cargo check          # Compile check
cargo test           # Run all tests
cargo run -- --status  # Verify the daemon starts
```

### Run HIRO Benchmark
```bash
# Null-condition baseline (required before crediting any evolution)
PROFESSOR_X_DATA_DIR="$PWD/.px-data-null" cargo run -- --hiro-null 3

# Single round
cargo run -- --hiro 0 --hiro-limit 1
```

## Contribution Areas

### 🧪 Research & Hypotheses
- Propose new hypotheses in `brain/hypotheses.md`
- Design experiments for existing hypotheses
- Review and discuss in [Research Discussions](../../issues?q=label%3Aresearch)

### 🔧 Core Harness
- Rust source lives in `professor-x/src/`
- Follow the daemon architecture: `memd`, `toolbridge`, `agentd`, `policyd`, `evolved`
- Every change must pass `cargo check` and `cargo test`
- Evolution changes must go through the verify-then-commit pipeline

### 📝 Documentation
- Architecture docs in the repo root (`ARCHITECTURE.md`, `MEMORY_ARCHITECTURE.md`)
- Brain files in `brain/` and `professor-x/brain/`
- Runbooks in `professor-x/ops/runbooks/`
- Skills in `professor-x/skills/`

### 🛡️ Safety & Policy
- Policy changes affect all autonomous behavior
- Test against `professor-x/tests/policy/` fixtures
- Document risk score changes in your PR

## Code Style

- **Rust**: Follow `rustfmt` defaults. Run `cargo fmt` before committing.
- **Clippy**: No warnings. Run `cargo clippy -- -D warnings`.
- **Commits**: Use clear, specific messages. "Fix DHE layer-3 attribution for circular reasoning" beats "fix bug".
- **Safety**: Never bypass the policy gate, audit chain, or approval queue in a PR.

## Verify-Then-Commit Protocol

All autonomous or evolution changes must:
1. **Propose** — generate a diff with motivation and target component
2. **Verify** — apply in an isolated sandbox worktree, run `cargo check` + targeted tests
3. **Scan** — check for reward hacking, no-op diffs, and policy violations
4. **Decide** — accept → commit, reject → rollback, or defer

Manual PRs don't need the full protocol, but must pass CI.

## Reporting Issues

- **Bug**: Use the [Bug Report](../../issues/new?template=bug_report.md) template
- **Feature**: Use the [Feature Request](../../issues/new?template=feature_request.md) template
- **Research**: Use the [Research Discussion](../../issues/new?template=research_discussion.md) template

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).

## Questions?

Open an issue or start a [Discussion](../../discussions). We respond within 48 hours.