#!/usr/bin/env python3
"""Frontier-Feel Scorecard — operationalize "does the local agent FEEL like a frontier agent?"

The goal (feel like OpenClaw/frontier but local in 12GB) is vague until measured. This turns a repo-fix
bench artifact into a measurable profile across the dimensions that make an agent *feel* frontier:
  - correctness   : pass@1 (does it actually fix the bug?)
  - reliability    : made_edit% (does it reliably drive the loop to an edit? — the no-edit class kills feel)
  - cleanliness    : 1 - wrong-edit-rate among edits (does its edit usually work, not flail?)
  - (transcript)   : thrash & steps if transcripts are present (smoothness/efficiency)
A composite FRONTIER-FEEL INDEX in [0,1] + a verdict vs a target bar.

Usage: frontier_feel_score.py <bench_artifact.json> [<artifact2> ...]
"""
import json, os, sys, glob

# What "feels frontier" means, concretely (tunable bar):
TARGET = {"correctness": 0.60, "reliability": 0.95, "cleanliness": 0.60}
WEIGHTS = {"correctness": 0.5, "reliability": 0.3, "cleanliness": 0.2}


def profile(art):
    d = json.load(open(art))
    ts = d.get("tasks", [])
    n = len(ts) or 1
    passed = sum(1 for t in ts if t.get("passed"))
    made = sum(1 for t in ts if t.get("made_edit"))
    edited_fail = sum(1 for t in ts if t.get("made_edit") and not t.get("passed"))
    correctness = passed / n
    reliability = made / n
    cleanliness = 1 - (edited_fail / made) if made else 0.0   # of edits, fraction that worked
    index = sum(WEIGHTS[k] * min(1.0, v / TARGET[k]) for k, v in
                {"correctness": correctness, "reliability": reliability, "cleanliness": cleanliness}.items())
    index = round(min(1.0, index), 3)
    return {"model": d.get("model", "?"), "n": n, "correctness": round(correctness, 3),
            "reliability": round(reliability, 3), "cleanliness": round(cleanliness, 3),
            "frontier_feel_index": index,
            "meets_bar": correctness >= TARGET["correctness"] and reliability >= TARGET["reliability"]}


def main():
    arts = sys.argv[1:]
    if not arts:
        print("usage: frontier_feel_score.py <artifact.json> ..."); sys.exit(2)
    print(f"{'model':24} {'n':>3} {'correct':>7} {'reliab':>7} {'clean':>6} {'FEEL':>5}  bar?")
    rows = []
    for a in arts:
        try:
            p = profile(a); rows.append(p)
            print(f"{p['model']:24} {p['n']:>3} {p['correctness']:>7} {p['reliability']:>7} "
                  f"{p['cleanliness']:>6} {p['frontier_feel_index']:>5}  {'YES' if p['meets_bar'] else 'no'}")
        except Exception as e:
            print(f"{a}: ERR {e}")
    print(f"\nTarget bar (feels frontier): correctness>={TARGET['correctness']}, "
          f"reliability(made_edit)>={TARGET['reliability']}, cleanliness>={TARGET['cleanliness']}.")
    print("FRONTIER-FEEL INDEX = weighted progress toward the bar (1.0 = meets/exceeds on all).")


if __name__ == "__main__":
    main()
