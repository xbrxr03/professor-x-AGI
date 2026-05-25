# px-daily-cycle

## Purpose
Master orchestration loop. Runs the full 7-hour autonomous research day.
Calls other conductor skills in sequence. Handles interruption and resume.

## Schedule
```
Morning    → px-daily-update (Telegram brief + X post #1)
Hours 1-2  → px-literature-search + px-synthesize
Hours 2-4  → px-write-section (findings, teaching content, paper progress)
Hours 4-6  → px-experiment-runner (local hardware experiments)
Hour 6-7   → px-self-review (score 1-10, update hypotheses)
End of day → px-daily-update (GitHub commit + X post #2 + Discord)
```

## Self-termination
After 5 consecutive idle rounds (no knowledge gained, no harness evolved):
git add -A && git commit -m "Auto-commit: self-termination after N idle cycles"
Clean shutdown with logged reason.

## Status: STUB — implement from K-Dense-AI SKILL.md format
