# Workflow — Claude + Codex parallel agents (via parallel-code + AGENTS.md)

How we run two coding agents (Claude Code + Codex CLI) in parallel on this repo.

## The model
- **AGENTS.md** is the brain/ledger (the cross-tool standard both agents read): rules, per-stream task
  checklist with check-offs, append-only log. Read it before starting; check your box + log when done.
- **parallel-code** (v1.10.0, GUI, `~/Applications/Parallel.Code-*.AppImage`) is the orchestration
  surface: it spawns each agent in its own **git worktree** (auto branch + worktree), shows them
  side-by-side, and merges back from a sidebar. It removes the manual `git worktree add` + terminal
  juggling. It does NOT make agents talk to each other — coordination stays in AGENTS.md + git.
- **You** are the router: point each agent at its brief (`CODEX_TASK_*.md`), relay "done", review/merge.

## Rules (enforced by convention, not the tool)
1. **File-disjoint streams** — each agent owns distinct files; never both edit the same file. (Claude →
   `src/` + Rust; Codex → `distill/` + measurement scripts.) Only AGENTS.md is shared (append-only).
2. **One worktree/branch per stream.** Let parallel-code create them going forward (don't hand-make
   worktrees for the same branch it manages).
3. **GPU is single-owner** — whoever runs a bench/training declares it in the AGENTS.md log; never two
   GPU jobs at once (Ollama runner crashes under contention).
4. **Discipline:** full `cargo test --bins` green before committing any `src/` change; verify-the-ruler
   (honest before/after, no fabricated wins). Producer commits+pushes its deliverable; consumer pulls.

## The loop
1. In parallel-code, create a task per stream → it spawns the agent in a fresh worktree off
   `prereboot-flywheel-prep`.
2. Tell each agent its brief (e.g. Codex: "do CODEX_TASK_P3.md").
3. Each works its disjoint files, logs to AGENTS.md, commits.
4. You relay completion; the integrating agent (usually Claude) pulls + runs the gate.
5. Merge each green stream from the parallel-code sidebar.

## Launch
```bash
chmod +x ~/Applications/Parallel.Code-*.AppImage && ~/Applications/Parallel.Code-*.AppImage
# point it at /home/abrar/professor-x-main-integrate ; Ctrl+N to create a task per stream
```

## Current state (2026-06-22)
- Merged into `prereboot-flywheel-prep`: Stream A (behavior retrieval, flag OFF), Stream B (taxonomy),
  Stream D (TGC gate). Suite 370/370 green.
- IN PROGRESS: Codex Stream E on `codex/p3-distill` (distillation; uncommitted) — must commit before any
  worktree restart. Then: D-integration (run `tgc_gate.py` on `profx-distilled-p3` vs stock).
