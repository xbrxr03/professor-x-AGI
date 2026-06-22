#!/usr/bin/env python3
"""Verify that all tensors in a safetensors model directory are finite."""

from __future__ import annotations

import argparse
import math
import pathlib
import sys


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("model_dir", type=pathlib.Path, help="Directory containing *.safetensors")
    return ap.parse_args()


def main() -> int:
    args = parse_args()
    try:
        import torch
        from safetensors import safe_open
    except ImportError as exc:
        sys.exit(f"missing dependency: {exc}")

    files = sorted(args.model_dir.glob("*.safetensors"))
    if not files:
        sys.exit(f"no *.safetensors found in {args.model_dir}")

    checked = 0
    for path in files:
        with safe_open(path, framework="pt", device="cpu") as handle:
            for key in handle.keys():
                tensor = handle.get_tensor(key)
                checked += 1
                if not torch.isfinite(tensor).all():
                    print(f"NONFINITE {path.name}:{key}")
                    return 1
    print(f"FINITE {checked} tensor(s) across {len(files)} safetensors file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
