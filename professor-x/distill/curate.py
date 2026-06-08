#!/usr/bin/env python3
"""Curate the trajectory corpus into a clean QLoRA training set.

Reads artifacts/trajectories/<date>/trajectories.jsonl (written by the harness on
every verified-correct task), filters and balances them, and emits a single
chat-format SFT file at distill/data/curated.jsonl.

Curation (per the self-distillation research doc):
  - verified-correct only (the collector already enforces this)
  - de-duplicate by task text (keep the shortest successful trajectory — the
    most efficient solution is the best lesson)
  - drop trajectories that are too short (<2 steps) or absurdly long (>30)
  - balance across task_type so weak categories are represented (ZPD: don't let
    the easy category dominate the lesson)

Usage:  python3 distill/curate.py [--max-per-type 400]
"""
import argparse
import glob
import json
import os
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # professor-x/
# Recursive: tolerate both the canonical artifacts/trajectories/ and any nested
# professor-x/artifacts/trajectories/ a stray subdir may have produced. The
# corpus is precious — never miss a trajectory file because of a path quirk.
TRAJ_GLOB = os.path.join(ROOT, "**", "trajectories", "*", "trajectories.jsonl")
OUT_DIR = os.path.join(HERE, "data")
OUT = os.path.join(OUT_DIR, "curated.jsonl")


def load_all():
    rows = []
    seen = set()
    for path in glob.glob(TRAJ_GLOB, recursive=True):
        real = os.path.realpath(path)
        if real in seen:
            continue
        seen.add(real)
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-per-type", type=int, default=400)
    args = ap.parse_args()

    rows = load_all()
    if not rows:
        print(f"No trajectories found at {TRAJ_GLOB}")
        print("Run the harness (--hiro / --mine) first to accumulate verified trajectories.")
        return

    # de-dup by task: keep the trajectory with the fewest steps (most efficient).
    best_by_task = {}
    for r in rows:
        if not r.get("verified"):
            continue
        steps = r.get("steps", 0)
        if steps < 2 or steps > 30:
            continue
        task = r.get("task", "")
        prev = best_by_task.get(task)
        if prev is None or steps < prev.get("steps", 1e9):
            best_by_task[task] = r

    # balance across task_type
    by_type = defaultdict(list)
    for r in best_by_task.values():
        by_type[r.get("task_type", "other")].append(r)

    curated = []
    for t, items in by_type.items():
        items.sort(key=lambda r: r.get("steps", 0))  # prefer concise
        curated.extend(items[: args.max_per_type])

    os.makedirs(OUT_DIR, exist_ok=True)
    with open(OUT, "w") as f:
        for r in curated:
            # emit just the messages array — the training format
            f.write(json.dumps({"messages": r["messages"]}) + "\n")

    print(f"Loaded {len(rows)} raw, {len(best_by_task)} unique verified tasks.")
    print("By type:", {t: len(v) for t, v in by_type.items()})
    print(f"Wrote {len(curated)} curated examples → {OUT}")
    if len(curated) < 100:
        print("\nWARNING: <100 examples. Distillation wants hundreds+. Run more HIRO/mine rounds.")


if __name__ == "__main__":
    main()
