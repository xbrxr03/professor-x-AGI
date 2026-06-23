#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
from pathlib import Path


def repo_root_from_script() -> Path:
    return Path(__file__).resolve().parents[1]


def runtime_skill_dirs(runtime_root: Path) -> list[Path]:
    return sorted(
        path
        for path in runtime_root.iterdir()
        if path.is_dir() and (path / "SKILL.md").exists()
    )


def copy_skill(src: Path, dest_root: Path, force: bool) -> str:
    dest = dest_root / src.name
    if dest.exists():
        if not force:
            return f"skip   {src.name} -> {dest} (exists)"
        shutil.rmtree(dest)
    shutil.copytree(src, dest)
    return f"install {src.name} -> {dest}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Install repo-tracked runtime skills into a target skill directory."
    )
    parser.add_argument(
        "--dest",
        default=str(Path.home() / ".codex" / "skills"),
        help="Target skill directory (default: ~/.codex/skills).",
    )
    parser.add_argument(
        "--skill",
        action="append",
        dest="skills",
        help="Install only the named skill. Repeatable.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing destination skill directories.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = repo_root_from_script()
    runtime_root = root / "skills" / "runtime"
    dest_root = Path(args.dest).expanduser().resolve()
    dest_root.mkdir(parents=True, exist_ok=True)

    skills = runtime_skill_dirs(runtime_root)
    if args.skills:
        want = set(args.skills)
        skills = [skill for skill in skills if skill.name in want]

    if not skills:
        print("No matching repo skills found to install.")
        return 1

    for skill in skills:
        print(copy_skill(skill, dest_root, args.force))

    print(f"Installed {len(skills)} repo skill(s) into {dest_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
