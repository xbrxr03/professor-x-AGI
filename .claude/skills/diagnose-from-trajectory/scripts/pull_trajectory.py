#!/usr/bin/env python3
"""Tier-3 helper for diagnose-from-trajectory.

Pull a Professor X agent trajectory from the state DB and print its action sequence +
failure signature, so a fix targets the REAL failure instead of a guess.

Usage:
  python3 pull_trajectory.py "<task-description-keyword>" [--failed] [--db PATH]

Examples:
  python3 pull_trajectory.py slugify --failed
  python3 pull_trajectory.py "evens" --db ~/.professor-x/state.db
"""
import argparse, json, os, sqlite3, sys


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("keyword", help="substring of the task description to match")
    ap.add_argument("--failed", action="store_true", help="prefer a task with outcome_score < 1")
    ap.add_argument("--db", default=os.path.expanduser("~/.professor-x/state.db"))
    a = ap.parse_args()

    if not os.path.exists(a.db):
        print(f"no state DB at {a.db} (set PROFESSOR_X_DATA_DIR or pass --db)", file=sys.stderr)
        return 2
    con = sqlite3.connect(a.db)
    con.row_factory = sqlite3.Row
    rows = con.execute(
        "SELECT * FROM task_runs WHERE description LIKE ? ORDER BY rowid DESC LIMIT 20",
        (f"%{a.keyword}%",),
    ).fetchall()
    if not rows:
        print(f"no task_runs matching '{a.keyword}'", file=sys.stderr)
        return 1
    pick = None
    if a.failed:
        pick = next((r for r in rows if (r["outcome_score"] or 0) < 1), None)
    pick = pick or rows[0]

    print(f"TASK: {pick['description'][:90]}")
    print(f"status={pick['status']} steps={pick['step_count']} attempts={pick['attempt_count']} "
          f"last_tool={pick['last_tool']} score={pick['outcome_score']} failure_mode={pick['failure_mode']}")
    print("--- action sequence (look for loops / no-edit / tool-rejects) ---")
    seq, dups = [], 0
    for e in con.execute(
        "SELECT event_type,payload FROM agent_events WHERE task_id=? ORDER BY rowid", (pick["task_id"],)
    ):
        try:
            p = json.loads(e["payload"]) if e["payload"] else {}
        except Exception:
            p = {}
        et = e["event_type"]
        if et in ("tool.requested", "tool.succeeded", "tool.failed",
                  "react.duplicate_action", "react.synthesis_finish"):
            tool = p.get("tool") or p.get("tool_name") or ""
            seq.append(f"{et.split('.')[-1]}:{tool}")
            if et == "react.duplicate_action":
                dups += 1
            if et == "tool.requested" and "edit" in str(tool):
                params = p.get("params") or {}
                if isinstance(params, dict) and params.get("new_text"):
                    print(f"  edit attempt: {str(params)[:140]}")
    print("  " + " -> ".join(seq[:24]))
    # heuristic signature
    sig = []
    if dups >= 3:
        sig.append("GREEDY-LOOP (repeated a blocked action)")
    if not any("edit" in s for s in seq):
        sig.append("NO-EDIT (never reached an edit tool)")
    if any(s == "failed:fs.hash_edit" for s in seq):
        sig.append("EDIT-TOOL-REJECT (hash_edit failed)")
    print("  failure signature:", ", ".join(sig) or "(read the sequence above)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
