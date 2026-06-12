#!/usr/bin/env python3
"""Automated diagnosis (M4 frontier, step 1): scan recent repo-fix FAILURES, classify each
trajectory's mechanical failure mode, aggregate, and point at the implicated harness component
+ a fix direction. This automates the human diagnosis step that lifted repo-fix 0.50->0.85.

Read-only and safe — it proposes WHERE to look, never mutates code. Pair the output with a
strong proposer (not the 8B) to author the diff, then gate it on --repo-fix-bench.

Usage:  python3 diagnose.py [--db PATH] [--limit N]
"""
import argparse, json, os, sqlite3, sys
from collections import Counter

# failure mode -> (implicated harness component, fix direction)
ROUTING = {
    "GREEDY-LOOP":      ("agentd/react.rs (duplicate handling / sampling temp)",
                         "escalate temperature on a duplicate-blocked retry; forceful nudge"),
    "NO-EDIT":          ("agentd/react.rs (finish gating) or the task prompt",
                         "reject a finish with zero edits; insist on an applied edit"),
    "EDIT-TOOL-REJECT": ("toolbridge/hashedit.rs or executor.rs (edit tools)",
                         "make the edit tool forgiving (line fallback); lint-gate guards"),
    "WRONG-EDIT":       ("model capability (edit content) — hardest",
                         "stronger model, or scaffold the edit (show line + ask for replacement)"),
    "HALLUCINATION":    ("agentd/react.rs (tool enforcement)",
                         "require a real tool result before finish; forbid fabricated answers"),
}


def classify(events) -> str:
    seq, dups, edit_failed, made_edit, finished = [], 0, False, False, False
    for et, payload in events:
        try:
            p = json.loads(payload) if payload else {}
        except Exception:
            p = {}
        tool = p.get("tool") or p.get("tool_name") or ""
        if et == "react.duplicate_action":
            dups += 1
        if et == "tool.failed" and "edit" in str(tool):
            edit_failed = True
        if et == "tool.requested" and "edit" in str(tool):
            made_edit = True
        if et in ("task.succeeded", "react.synthesis_finish") or (et == "tool.requested" and tool in ("finish", "done")):
            finished = True
        seq.append((et, tool))
    if dups >= 3 and not made_edit:
        return "GREEDY-LOOP"
    if edit_failed:
        return "EDIT-TOOL-REJECT"
    if not made_edit:
        return "NO-EDIT"
    return "WRONG-EDIT"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=os.path.expanduser("~/.professor-x/state.db"))
    ap.add_argument("--limit", type=int, default=40)
    a = ap.parse_args()
    if not os.path.exists(a.db):
        print(f"no state DB at {a.db}", file=sys.stderr)
        return 2
    con = sqlite3.connect(a.db)
    con.row_factory = sqlite3.Row
    rows = con.execute(
        "SELECT task_id,description,outcome_score FROM task_runs "
        "WHERE (description LIKE '%files are in%' OR description LIKE '%Fix it%' OR description LIKE '%fix%') "
        "ORDER BY rowid DESC LIMIT ?", (a.limit,)).fetchall()
    modes = Counter()
    seen = set()
    for r in rows:
        if r["task_id"] in seen:
            continue
        seen.add(r["task_id"])
        if (r["outcome_score"] or 0) >= 1:
            continue  # only failures
        evs = con.execute(
            "SELECT event_type,payload FROM agent_events WHERE task_id=? ORDER BY rowid",
            (r["task_id"],)).fetchall()
        modes[classify([(e["event_type"], e["payload"]) for e in evs])] += 1

    if not modes:
        print("No recent repo-fix FAILURES found (all passing, or no trajectories).")
        return 0
    print("=== AUTOMATED DIAGNOSIS — recent repo-fix failures ===")
    for mode, n in modes.most_common():
        comp, fix = ROUTING.get(mode, ("(unknown)", "read the trajectory"))
        print(f"\n[{n:2}x] {mode}")
        print(f"      implicated component: {comp}")
        print(f"      fix direction:        {fix}")
    top = modes.most_common(1)[0][0]
    print(f"\n>>> DOMINANT failure: {top}. Author a scoped diff for its component, "
          f"have a human approve it, then gate on --repo-fix-bench.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
