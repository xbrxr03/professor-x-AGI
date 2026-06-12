---
name: diagnose-from-trajectory
description: "Before proposing a fix for an agent/harness failure on Professor X, READ the actual failing trajectory — never guess. Use whenever a HIRO/repo-fix task fails, pass@1 stalls, or you're about to 'improve' a prompt/tool. Guessed fixes (synthesis, self-verify prompt) failed; trajectory-diagnosed fixes (temp escalation, forgiving hash-edit) lifted repo-fix 0.50→0.77."
---

# Diagnose from the real trajectory, don't guess

The fixes that worked this session came from *reading what the agent actually did*; the fixes
that failed (or hurt) came from guessing. This is the strongest evolution signal on the project
— stronger than blind LLM prompt-proposal (which proposed *worse* prompts).

## How to read a trajectory

The agent's steps are in the SQLite DB (`$PROFESSOR_X_DATA_DIR/state.db`, default `~/.professor-x/`),
queryable with Python's `sqlite3` (the CLI isn't installed):

```python
import sqlite3, os, json
con = sqlite3.connect(os.path.expanduser("~/.professor-x/state.db")); con.row_factory = sqlite3.Row
# tables: task_runs (status/last_tool/failure_mode/outcome_score), agent_events (full step stream)
row = con.execute("SELECT * FROM task_runs WHERE description LIKE ? ORDER BY rowid DESC LIMIT 1", ("%<keyword>%",)).fetchone()
for e in con.execute("SELECT event_type,payload FROM agent_events WHERE task_id=? ORDER BY rowid", (row['task_id'],)):
    p = json.loads(e['payload'] or '{}')
    print(e['event_type'], p.get('tool',''), str(p.get('params') or p.get('preview') or '')[:120])
```

Also: the repo-fix runner captures the agent's diff on failure (`<none>` = made no edit). The
attempt artifacts persist the final answer (M0.2b) so verdicts are auditable.

## What to look for (the measured failure modes)
- **Greedy decode-loop**: the agent re-emits the *identical* thought+action (e.g. `fs.list`)
  forever → fixed by escalating temperature on a duplicate-blocked retry.
- **No edit made**: it gathers/thrashes then finishes without editing → the dominant repo-fix miss.
- **Tool rejects a correct action**: e.g. the 8B invents a line-hash, strict `hash_edit` rejects
  a correct fix → made forgiving (line-based fallback, lint-gated).
- **Hallucination**: fabricates results instead of using a tool.

## Rule
Identify the *specific* failure from the trace, fix *that*, then re-measure (see
`verify-the-ruler`). A flat metric after a fix often means the bottleneck just moved one stage
later — re-read the trajectory to find the new wall.
