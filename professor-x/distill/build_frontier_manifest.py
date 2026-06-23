#!/usr/bin/env python3
"""Build a repo-fix manifest for the Phase 3 wrong-edit frontier.

Frontier definition for Stream E:
  - student model: qwen3:8b-q4_K_M
  - source: failure-taxonomy JSON from Stream B
  - default bucket filter: wrong-edit-verified-fail

This writes a standard repo-fix tasks manifest containing only the selected task
records from hard + family manifests, so the native bench can collect teacher
trajectories directly on the frontier.
"""

from __future__ import annotations

import argparse
import json
import pathlib
from collections import Counter

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
REPO_FIX = ROOT / "scripts" / "benchmarks" / "repo_fix"
DEFAULT_TAXONOMY = (
    ROOT.parent.parent
    / "px-codex-measure"
    / "professor-x"
    / "artifacts"
    / "repo-fix"
    / "failure-taxonomy-2026-06-22.json"
)
MANIFESTS = [
    REPO_FIX / "tasks_hard_full.json",
    REPO_FIX / "tasks_family_csv.json",
    REPO_FIX / "tasks_family_graph.json",
    REPO_FIX / "tasks_family_interval.json",
    REPO_FIX / "tasks_family_money.json",
    REPO_FIX / "tasks_family_sm.json",
    REPO_FIX / "tasks_family_stack.json",
    REPO_FIX / "tasks_family_unit.json",
]


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--taxonomy-json",
        type=pathlib.Path,
        default=DEFAULT_TAXONOMY,
        help="Failure-taxonomy JSON to mine for student failures",
    )
    ap.add_argument(
        "--student-model",
        default="qwen3:8b-q4_K_M",
        help="Model name to treat as the failing student",
    )
    ap.add_argument(
        "--bucket",
        action="append",
        dest="buckets",
        default=[],
        help="Failure bucket(s) to include; repeat to include multiple. Defaults to wrong-edit-verified-fail.",
    )
    ap.add_argument(
        "--out",
        type=pathlib.Path,
        default=HERE / "data" / "frontier_wrong_edit_p3.json",
        help="Output manifest path",
    )
    return ap.parse_args()


def load_json(path: pathlib.Path) -> dict | list:
    with path.open() as fh:
        return json.load(fh)


def load_task_index() -> dict[str, dict]:
    tasks: dict[str, dict] = {}
    for manifest in MANIFESTS:
        payload = load_json(manifest)
        for task in payload["tasks"]:
            task_id = task["id"]
            if task_id in tasks:
                raise RuntimeError(f"duplicate task id across manifests: {task_id}")
            tasks[task_id] = task
    return tasks


def main() -> int:
    args = parse_args()
    buckets = set(args.buckets or ["wrong-edit-verified-fail"])
    taxonomy_rows = load_json(args.taxonomy_json)
    if not isinstance(taxonomy_rows, list):
        raise RuntimeError(f"unexpected taxonomy payload at {args.taxonomy_json}")

    selected_ids: list[str] = []
    by_task_set: Counter[str] = Counter()
    for row in taxonomy_rows:
        if row.get("model") != args.student_model:
            continue
        for failure in row.get("failures", []):
            if failure.get("bucket") not in buckets:
                continue
            task_id = failure["task_id"]
            selected_ids.append(task_id)
            by_task_set[row["task_set"]] += 1

    task_index = load_task_index()
    selected_tasks = []
    missing = []
    for task_id in selected_ids:
        task = task_index.get(task_id)
        if task is None:
            missing.append(task_id)
            continue
        selected_tasks.append(task)
    if missing:
        raise RuntimeError(f"task ids missing from manifests: {missing}")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "generated_from": str(args.taxonomy_json),
        "student_model": args.student_model,
        "buckets": sorted(buckets),
        "tasks": selected_tasks,
    }
    args.out.write_text(json.dumps(payload, indent=2) + "\n")

    print(f"frontier tasks: {len(selected_tasks)}")
    print(f"by task set: {dict(by_task_set)}")
    print(f"output: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
