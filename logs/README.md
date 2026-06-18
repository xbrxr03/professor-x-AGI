# 📓 Build Logs — Professor X

A public, honest record of how this project is actually built: the problems we hit, the decisions
we make, and the solutions we land on. Build-in-public, including the ugly parts.

Two kinds of log:

- **Devlog entries** (`logs/YYYY-MM-DD-<slug>.md`) — what happened on a given day/session: the
  problems, their root causes, and the fixes. Reverse-chronological. Each problem is logged as
  **Symptom → Root cause → Fix → Status** so it's scannable (and reel-friendly).
- **[DECISIONS.md](DECISIONS.md)** — the running list of design decisions and principles we adopt,
  with the *why*. When a devlog produces a durable rule, it graduates here.

## Why this exists
1. **Honesty / eval-trust.** This project has shipped a fabricated "win" before. Writing problems
   and results down publicly — including the rejects — is part of the discipline that prevents it.
2. **Build-in-public.** The real story (a $400 GPU, one person, a debugging gauntlet) is the content.
3. **Future-us.** Every problem here cost hours. Logged, it costs minutes next time.

## Index
- [2026-06-17 — The distillation gauntlet](2026-06-17-distillation-gauntlet.md): got the
  distillation flywheel working end-to-end on the RTX 3060; the bug was never the ML, it was the
  plumbing (serving template + train/serve format). First real gate verdict: REJECT-by-ceiling.
