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

Usage:  python3 distill/curate.py [--max-per-type 400] [--glob 'artifacts/trajectories/2026-06-22/*.jsonl']
"""
import argparse
import glob
import json
import os
import re
from collections import defaultdict
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # professor-x/
# Recursive: tolerate both the canonical artifacts/trajectories/ and any nested
# professor-x/artifacts/trajectories/ a stray subdir may have produced. The
# corpus is precious — never miss a trajectory file because of a path quirk.
TRAJ_GLOB = os.path.join(ROOT, "**", "trajectories", "*", "trajectories.jsonl")
OUT_DIR = os.path.join(HERE, "data")
OUT = os.path.join(OUT_DIR, "curated.jsonl")
TASK_ID_RE = re.compile(r"px-repofix-([a-z0-9_]+)-")


def load_all(traj_glob: str):
    rows = []
    seen = set()
    for path in glob.glob(traj_glob, recursive=True):
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


def load_manifest_task_index(paths):
    index = {}
    for raw_path in paths:
        path = Path(raw_path)
        with path.open() as f:
            payload = json.load(f)
        tasks = payload.get("tasks", payload if isinstance(payload, list) else [])
        for task in tasks:
            description = task.get("description", "").strip()
            task_id = task.get("id", description)
            if description:
                index[description] = task_id
    return index


def row_task_text(row):
    parts = [row.get("task", "")]
    for msg in row.get("messages", []):
        if msg.get("role") == "user":
            parts.append(msg.get("content", ""))
    return "\n".join(part for part in parts if part)


def manifest_match(row, description_to_id):
    if not description_to_id:
        return None
    haystack = row_task_text(row)
    for description, task_id in description_to_id.items():
        if description in haystack:
            return task_id
    return None


def manifest_task_key(row):
    match = TASK_ID_RE.search(row_task_text(row))
    if match:
        return match.group(1)
    return row.get("task", "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-per-type", type=int, default=400)
    ap.add_argument(
        "--glob",
        default=TRAJ_GLOB,
        help="Glob for trajectory jsonl files to curate (default: all trajectories under the repo)",
    )
    ap.add_argument(
        "--out",
        default=OUT,
        help="Destination jsonl path for curated training rows",
    )
    ap.add_argument(
        "--manifest",
        action="append",
        default=[],
        help="Optional repo-fix manifest(s). When set, keep only trajectories whose task text matches a manifest description.",
    )
    args = ap.parse_args()

    rows = load_all(args.glob)
    if not rows:
        print(f"No trajectories found at {args.glob}")
        print("Run the harness (--hiro / --mine) first to accumulate verified trajectories.")
        return

    description_to_id = load_manifest_task_index(args.manifest)
    if args.manifest:
        print(
            f"Manifest filter active: {len(description_to_id)} task description(s) from {len(args.manifest)} manifest(s)."
        )

    # de-dup by task: keep the trajectory with the fewest steps (most efficient).
    best_by_task = {}
    filtered_out = 0
    for r in rows:
        if not r.get("verified"):
            continue
        steps = r.get("steps", 0)
        if steps < 2 or steps > 30:
            continue
        matched_task_id = manifest_match(r, description_to_id)
        if args.manifest and matched_task_id is None:
            filtered_out += 1
            continue
        task = manifest_task_key(r) if args.manifest else (matched_task_id or r.get("task", ""))
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

    out = os.path.abspath(args.out)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        for r in curated:
            # emit just the messages array — the training format
            f.write(json.dumps({"messages": r["messages"]}) + "\n")

    print(f"Loaded {len(rows)} raw, {len(best_by_task)} unique verified tasks.")
    print("By type:", {t: len(v) for t, v in by_type.items()})
    if args.manifest:
        print(f"Filtered out {filtered_out} non-manifest trajectories.")
    print(f"Wrote {len(curated)} curated examples → {out}")
    if len(curated) < 100:
        print("\nWARNING: <100 examples. Distillation wants hundreds+. Run more HIRO/mine rounds.")


if __name__ == "__main__":
    main()
