#!/usr/bin/env python3
"""Slice a repo-fix manifest by pass/fail status from a recorded artifact."""

from __future__ import annotations

import argparse
import json
import pathlib


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--artifact", required=True, type=pathlib.Path, help="repo-fix artifact JSON")
    ap.add_argument("--manifest", required=True, type=pathlib.Path, help="Source tasks manifest JSON")
    ap.add_argument("--select", choices=["passed", "failed"], required=True, help="Task subset to emit")
    ap.add_argument("--out", required=True, type=pathlib.Path, help="Destination manifest JSON")
    return ap.parse_args()


def load_json(path: pathlib.Path) -> dict:
    with path.open() as fh:
        return json.load(fh)


def main() -> int:
    args = parse_args()
    artifact = load_json(args.artifact)
    manifest = load_json(args.manifest)

    wanted_ids = {
        task["id"]
        for task in artifact["tasks"]
        if bool(task.get("passed")) == (args.select == "passed")
    }
    selected = [task for task in manifest["tasks"] if task["id"] in wanted_ids]

    payload = dict(manifest)
    payload["tasks"] = selected
    payload["derived_from_artifact"] = str(args.artifact)
    payload["selection"] = args.select

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"{args.select} tasks: {len(selected)}")
    print(f"output: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
