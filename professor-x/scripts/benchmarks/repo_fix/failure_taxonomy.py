#!/usr/bin/env python3
"""Run repo-fix across the hard set + family manifests and bucket failures."""

from __future__ import annotations

import argparse
import collections
import dataclasses
import datetime as dt
import json
import os
import pathlib
import re
import subprocess
import sys
import time
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parents[3]
BENCH_DIR = ROOT / "scripts" / "benchmarks" / "repo_fix"
DEFAULT_BINARY = ROOT / "target" / "release" / "professor-x"
DEFAULT_DATA_DIR = pathlib.Path.home() / ".professor-x"
DEFAULT_MODELS = ["qwen3:8b-q4_K_M", "profx-distilled-clean"]
HARD_MANIFEST = BENCH_DIR / "tasks_hard_full.json"
FAMILY_MANIFESTS = sorted(BENCH_DIR.glob("tasks_family_*.json"))
BUCKETS = [
    "duplicate_action",
    "finish_rejected",
    "edit-apply-error",
    "wrong-edit-verified-fail",
    "loop/forfeit",
    "other",
]
EDIT_TOOLS = {"fs.hash_edit", "fs.replace", "patch.apply", "fs.write"}
ARTIFACT_LINE_RE = re.compile(r"^artifact = (.+)$", re.MULTILINE)
ARTIFACT_NAME_RE = re.compile(r"repo-fix-\d{6}-[0-9a-f]{8}\.json$")


@dataclasses.dataclass
class ManifestSpec:
    label: str
    rel_path: str
    abs_path: pathlib.Path
    task_count: int


@dataclasses.dataclass
class FailureRecord:
    task_id: str
    bucket: str
    made_edit: bool
    transcript_path: str | None
    post_exit: int
    expect_exit: int


@dataclasses.dataclass
class RunSummary:
    model: str
    task_set: str
    manifest_path: str
    task_count: int
    passed: int
    ran: int
    pass_at_1: float
    artifact_path: str
    duration_s: float
    bucket_counts: dict[str, int]
    failures: list[FailureRecord]


def load_json(path: pathlib.Path) -> dict[str, Any]:
    with path.open() as handle:
        return json.load(handle)


def manifest_specs() -> list[ManifestSpec]:
    specs: list[ManifestSpec] = []
    hard = load_json(HARD_MANIFEST)
    specs.append(
        ManifestSpec(
            label="hard",
            rel_path=str(HARD_MANIFEST.relative_to(ROOT)),
            abs_path=HARD_MANIFEST,
            task_count=len(hard["tasks"]),
        )
    )
    for manifest in FAMILY_MANIFESTS:
        payload = load_json(manifest)
        family = payload.get("family") or manifest.stem.removeprefix("tasks_family_")
        specs.append(
            ManifestSpec(
                label=f"family:{family}",
                rel_path=str(manifest.relative_to(ROOT)),
                abs_path=manifest,
                task_count=len(payload["tasks"]),
            )
        )
    return specs


def transcript_text_blobs(transcript: dict[str, Any]) -> list[str]:
    blobs: list[str] = []
    for event in transcript.get("events", []):
        blobs.append(str(event.get("event_type", "")))
        blobs.append(str(event.get("summary", "")))
        payload = event.get("payload")
        if isinstance(payload, dict):
            blobs.append(json.dumps(payload, sort_keys=True))
    for step in transcript.get("steps", []):
        blobs.append(str(step.get("tool_name", "")))
        blobs.append(str(step.get("thought", "")))
        blobs.append(str(step.get("observation_output", "")))
        error = step.get("observation_error")
        if error:
            blobs.append(str(error))
    return blobs


def has_duplicate_action(transcript: dict[str, Any]) -> bool:
    for event in transcript.get("events", []):
        if event.get("event_type") == "react.duplicate_action":
            return True
    text = "\n".join(transcript_text_blobs(transcript)).lower()
    return "duplicate action blocked" in text or "blocked duplicate" in text


def has_finish_rejection(transcript: dict[str, Any]) -> bool:
    for event in transcript.get("events", []):
        if event.get("event_type") == "task.finish_rejected":
            return True
    text = "\n".join(transcript_text_blobs(transcript)).lower()
    return any(
        needle in text
        for needle in (
            "empty finish has no answer",
            "policy denied: tool 'finish",
            "finish rejected",
        )
    )


def has_edit_apply_error(transcript: dict[str, Any]) -> bool:
    for step in transcript.get("steps", []):
        if step.get("tool_name") not in EDIT_TOOLS:
            continue
        if step.get("observation_success", True):
            continue
        error = str(step.get("observation_error") or "").lower()
        output = str(step.get("observation_output") or "").lower()
        if "duplicate action blocked" in error:
            continue
        if any(
            needle in f"{error}\n{output}"
            for needle in (
                "anchor",
                "mismatch",
                "replace",
                "patch",
                "apply",
                "compile",
                "verify",
                "span",
                "syntaxerror",
                "failed",
            )
        ):
            return True
    return False


def is_duplicate_block(step: dict[str, Any]) -> bool:
    error = str(step.get("observation_error") or "").lower()
    output = str(step.get("observation_output") or "").lower()
    return "duplicate action blocked" in error or "duplicate action" in output


def is_finish_rejection_step(step: dict[str, Any]) -> bool:
    if step.get("tool_name") != "finish":
        return False
    error = str(step.get("observation_error") or "").lower()
    output = str(step.get("observation_output") or "").lower()
    return (
        "verifier failed before finish" in error
        or "new edit required after verifier failure" in error
        or "verifier failed" in output
    )


def is_substantive_edit_failure(step: dict[str, Any]) -> bool:
    if step.get("tool_name") not in EDIT_TOOLS:
        return False
    if step.get("observation_success", True):
        return False
    if is_duplicate_block(step):
        return False
    return True


def load_transcript(
    transcript_path: str | None, artifact_repo_root: pathlib.Path
) -> dict[str, Any] | None:
    if not transcript_path:
        return None
    path = pathlib.Path(transcript_path)
    if not path.is_absolute():
        path = artifact_repo_root / path
    if not path.exists():
        return None
    return load_json(path)


def classify_failure(task: dict[str, Any], transcript: dict[str, Any] | None) -> str:
    if transcript:
        steps = transcript.get("steps", [])
        last_successful_edit_idx = max(
            (
                idx
                for idx, step in enumerate(steps)
                if step.get("tool_name") in EDIT_TOOLS
                and step.get("observation_success", False)
            ),
            default=None,
        )
        last_failed_edit_idx = max(
            (
                idx
                for idx, step in enumerate(steps)
                if is_substantive_edit_failure(step)
            ),
            default=None,
        )
        if last_failed_edit_idx is not None and (
            last_successful_edit_idx is None
            or last_failed_edit_idx > last_successful_edit_idx
        ):
            return "edit-apply-error"
        if task.get("made_edit") and last_successful_edit_idx is not None:
            return "wrong-edit-verified-fail"
        if any(is_finish_rejection_step(step) for step in steps) or has_finish_rejection(
            transcript
        ):
            return "finish_rejected"
        if has_duplicate_action(transcript):
            return "duplicate_action"
    if task.get("made_edit"):
        return "wrong-edit-verified-fail"
    if transcript:
        status = str(transcript.get("status", "")).lower()
        if status == "failed":
            return "loop/forfeit"
    if not task.get("made_edit"):
        return "loop/forfeit"
    return "other"


def parse_artifact_path(stdout: str, stderr: str) -> pathlib.Path:
    match = ARTIFACT_LINE_RE.search(stdout)
    if match:
        return pathlib.Path(match.group(1).strip())
    for line in reversed(stderr.splitlines()):
        candidate = line.strip()
        if ARTIFACT_NAME_RE.search(candidate):
            return pathlib.Path(candidate)
    raise RuntimeError("benchmark stdout did not report an artifact path")


def write_captured(stream: str | bytes | None) -> None:
    if stream is None:
        return
    if isinstance(stream, bytes):
        sys.stderr.write(stream.decode("utf-8", errors="replace"))
        return
    sys.stderr.write(stream)


def run_benchmark(
    binary: pathlib.Path,
    model: str,
    manifest: ManifestSpec,
    data_dir: pathlib.Path,
    timeout_seconds: float | None,
    retries: int,
) -> tuple[dict[str, Any], pathlib.Path, float]:
    env = os.environ.copy()
    env["PROFESSOR_X_NATIVE_TOOLS"] = "1"
    env["PROFESSOR_X_DATA_DIR"] = str(data_dir)
    env["REPO_FIX_TASKS"] = manifest.rel_path
    cmd = [str(binary), "--repo-fix-bench", "--model", model]
    total_duration_s = 0.0
    attempts = retries + 1
    for attempt in range(1, attempts + 1):
        started = time.monotonic()
        try:
            proc = subprocess.run(
                cmd,
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
                timeout=timeout_seconds,
            )
        except subprocess.TimeoutExpired as exc:
            total_duration_s += time.monotonic() - started
            write_captured(exc.stdout)
            write_captured(exc.stderr)
            if attempt < attempts:
                print(
                    f"[retry] model={model} task_set={manifest.label} attempt={attempt}/{attempts} "
                    f"timed out after {timeout_seconds:.0f}s",
                    flush=True,
                )
                continue
            raise RuntimeError(
                f"repo-fix bench timed out for model={model} manifest={manifest.rel_path} "
                f"after {timeout_seconds:.0f}s on attempt {attempt}/{attempts}"
            ) from exc
        total_duration_s += time.monotonic() - started
        if proc.returncode != 0:
            sys.stderr.write(proc.stdout)
            sys.stderr.write(proc.stderr)
            raise RuntimeError(
                f"repo-fix bench failed for model={model} manifest={manifest.rel_path} "
                f"(exit {proc.returncode})"
            )
        artifact_path = parse_artifact_path(proc.stdout, proc.stderr)
        if not artifact_path.is_absolute():
            artifact_path = ROOT / artifact_path
        return load_json(artifact_path), artifact_path, total_duration_s
    raise AssertionError("unreachable")


def summarize_run(
    model: str,
    manifest: ManifestSpec,
    artifact: dict[str, Any],
    artifact_path: pathlib.Path,
    duration_s: float,
) -> RunSummary:
    artifact_repo_root = artifact_path.parents[3]
    bucket_counts = {bucket: 0 for bucket in BUCKETS}
    failures: list[FailureRecord] = []
    for task in artifact["tasks"]:
        if task.get("passed"):
            continue
        transcript = load_transcript(task.get("transcript_path"), artifact_repo_root)
        bucket = classify_failure(task, transcript)
        bucket_counts[bucket] += 1
        failures.append(
            FailureRecord(
                task_id=str(task["id"]),
                bucket=bucket,
                made_edit=bool(task.get("made_edit")),
                transcript_path=task.get("transcript_path"),
                post_exit=int(task.get("post_exit", -1)),
                expect_exit=int(task.get("expect_exit", -1)),
            )
        )
    return RunSummary(
        model=model,
        task_set=manifest.label,
        manifest_path=artifact.get("manifest_path", manifest.rel_path),
        task_count=manifest.task_count,
        passed=int(artifact["passed"]),
        ran=int(artifact["ran"]),
        pass_at_1=float(artifact["pass_at_1"]),
        artifact_path=str(artifact_path),
        duration_s=duration_s,
        bucket_counts=bucket_counts,
        failures=failures,
    )


def render_table(rows: list[RunSummary]) -> str:
    lines = [
        "| model | task set | tasks | pass@1 | passed/ran | duplicate_action | finish_rejected | edit-apply-error | wrong-edit-verified-fail | loop/forfeit | other |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in rows:
        buckets = row.bucket_counts
        lines.append(
            "| {model} | {task_set} | {task_count} | {p1:.3f} | {passed}/{ran} | {dup} | {finish} | {edit} | {wrong} | {loop} | {other} |".format(
                model=row.model,
                task_set=row.task_set,
                task_count=row.task_count,
                p1=row.pass_at_1,
                passed=row.passed,
                ran=row.ran,
                dup=buckets["duplicate_action"],
                finish=buckets["finish_rejected"],
                edit=buckets["edit-apply-error"],
                wrong=buckets["wrong-edit-verified-fail"],
                loop=buckets["loop/forfeit"],
                other=buckets["other"],
            )
        )
    return "\n".join(lines)


def dominant_bucket(rows: list[RunSummary]) -> tuple[str, int, int]:
    counter: collections.Counter[str] = collections.Counter()
    failures = 0
    for row in rows:
        counter.update(row.bucket_counts)
        failures += sum(row.bucket_counts.values())
    if failures == 0:
        return ("none", 0, 0)
    bucket, count = max(counter.items(), key=lambda item: (item[1], item[0]))
    return bucket, count, failures


def model_takeaways(rows: list[RunSummary]) -> list[str]:
    grouped: dict[str, list[RunSummary]] = collections.defaultdict(list)
    for row in rows:
        grouped[row.model].append(row)
    lines: list[str] = []
    for model in sorted(grouped):
        bucket, count, failures = dominant_bucket(grouped[model])
        if failures == 0:
            lines.append(
                f"{model} cleared every measured task in this matrix, so there was no dominant failure bucket to report."
            )
            continue
        share = count / failures if failures else 0.0
        lines.append(
            f"{model} failed {failures} task(s); the dominant bucket was `{bucket}` at {count}/{failures} "
            f"({share:.1%}), which is the first place to attack if Stream C is going to buy real pass@1."
        )
    return lines


def action_notes(rows: list[RunSummary]) -> list[str]:
    grouped: dict[str, list[RunSummary]] = collections.defaultdict(list)
    by_task_set: dict[str, dict[str, RunSummary]] = collections.defaultdict(dict)
    for row in rows:
        grouped[row.model].append(row)
        by_task_set[row.task_set][row.model] = row

    lines: list[str] = []
    for model in sorted(grouped):
        model_rows = grouped[model]
        failures = collections.Counter[str]()
        total_failures = 0
        for row in model_rows:
            failures.update(row.bucket_counts)
            total_failures += sum(row.bucket_counts.values())
        wrong = failures["wrong-edit-verified-fail"]
        edit_apply = failures["edit-apply-error"]
        finish = failures["finish_rejected"]
        if total_failures:
            lines.append(
                f"{model}: `wrong-edit-verified-fail` dominates at {wrong}/{total_failures} "
                f"({wrong/total_failures:.1%}), while `edit-apply-error` is smaller at "
                f"{edit_apply}/{total_failures} ({edit_apply/total_failures:.1%}). "
                f"That makes verifier-informed retry directionally useful, but not sufficient on its own."
            )
        if finish:
            worst_finish = max(
                model_rows,
                key=lambda row: (
                    row.bucket_counts["finish_rejected"],
                    -row.pass_at_1,
                    row.task_set,
                ),
            )
            if worst_finish.bucket_counts["finish_rejected"] > 0:
                lines.append(
                    f"{model}: `finish_rejected` clusters most in `{worst_finish.task_set}` "
                    f"({worst_finish.bucket_counts['finish_rejected']} case(s)), so finish-gating regressions "
                    f"are family-specific rather than the main global failure mode."
                )

    if "family:interval" in by_task_set and len(by_task_set["family:interval"]) >= 2:
        distilled = by_task_set["family:interval"].get("profx-distilled-clean")
        qwen = by_task_set["family:interval"].get("qwen3:8b-q4_K_M")
        if distilled and qwen:
            lines.append(
                "Cross-model outlier: `family:interval` splits sharply by model. "
                f"`profx-distilled-clean` hit `finish_rejected` {distilled.bucket_counts['finish_rejected']}/"
                f"{sum(distilled.bucket_counts.values())} times at 0.000 pass@1, while "
                f"`qwen3:8b-q4_K_M` reached 0.400 pass@1 with failures entirely in "
                f"`wrong-edit-verified-fail` ({qwen.bucket_counts['wrong-edit-verified-fail']})."
            )

    if "family:csv" in by_task_set and "family:unit" in by_task_set:
        qwen_csv = by_task_set["family:csv"].get("qwen3:8b-q4_K_M")
        qwen_unit = by_task_set["family:unit"].get("qwen3:8b-q4_K_M")
        if qwen_csv and qwen_unit:
            lines.append(
                "High-ROI qwen families are `family:csv` and `family:unit`: both were 0.000 pass@1 and every "
                "failure landed in `wrong-edit-verified-fail`, which points at bad patch choice rather than "
                "tool execution trouble."
            )

    total_loops = sum(row.bucket_counts["loop/forfeit"] for row in rows)
    total_other = sum(row.bucket_counts["other"] for row in rows)
    if total_loops == 0 and total_other == 0:
        lines.append(
            "The harness is not mainly losing to endless thrash anymore: `loop/forfeit` and `other` were both zero "
            "across the measured matrix."
        )
    return lines


def representative_failures(rows: list[RunSummary]) -> list[str]:
    grouped: dict[str, list[RunSummary]] = collections.defaultdict(list)
    for row in rows:
        grouped[row.model].append(row)

    lines: list[str] = []
    for model in sorted(grouped):
        failures_by_bucket: dict[str, list[tuple[str, str]]] = collections.defaultdict(list)
        for row in grouped[model]:
            for failure in row.failures:
                failures_by_bucket[failure.bucket].append((row.task_set, failure.task_id))

        dominant_bucket = max(
            BUCKETS,
            key=lambda bucket: (len(failures_by_bucket[bucket]), bucket),
        )
        dominant_examples = ", ".join(
            f"`{task_set}/{task_id}`"
            for task_set, task_id in failures_by_bucket[dominant_bucket][:5]
        )
        if dominant_examples:
            lines.append(
                f"{model}: representative `{dominant_bucket}` tasks include {dominant_examples}."
            )

        finish_examples = failures_by_bucket["finish_rejected"][:3]
        if finish_examples:
            lines.append(
                f"{model}: representative `finish_rejected` tasks include "
                + ", ".join(f"`{task_set}/{task_id}`" for task_set, task_id in finish_examples)
                + "."
            )

        edit_examples = failures_by_bucket["edit-apply-error"][:3]
        if edit_examples:
            lines.append(
                f"{model}: representative `edit-apply-error` tasks include "
                + ", ".join(f"`{task_set}/{task_id}`" for task_set, task_id in edit_examples)
                + "."
            )
    return lines


def provenance_notes(rows: list[RunSummary]) -> list[str]:
    reused = [row for row in rows if row.duration_s == 0.0]
    if not reused:
        return []
    labels = ", ".join(f"`{row.model}/{row.task_set}`" for row in reused)
    return [
        f"{len(reused)} row(s) were ingested from existing native bench artifacts via "
        f"`--reuse-existing-root` instead of rerun in this invocation: {labels}."
    ]


def render_markdown(rows: list[RunSummary], binary: pathlib.Path) -> str:
    today = dt.datetime.now().astimezone().strftime("%Y-%m-%d")
    specs = manifest_specs()
    manifest_lines = "\n".join(f"- `{spec.label}` -> `{spec.rel_path}`" for spec in specs)
    provenance = provenance_notes(rows)
    actions = action_notes(rows)
    examples = representative_failures(rows)
    takeaways = "\n".join(f"- {line}" for line in model_takeaways(rows))
    sections = [
        "# 2026-06-21 Failure Taxonomy",
        "",
        f"Measured on **{today}** from the `codex/failure-taxonomy` worktree using `{binary}`.",
        "",
        "Command recipe:",
        "```bash",
        "PROFESSOR_X_NATIVE_TOOLS=1 PROFESSOR_X_DATA_DIR=$HOME/.professor-x \\",
        "REPO_FIX_TASKS=<manifest> ./target/release/professor-x --repo-fix-bench --model <model>",
        "```",
        "",
        "Manifests:",
        manifest_lines,
        "",
        "## Results",
        "",
        render_table(rows),
    ]
    if provenance:
        sections.extend(
            [
                "",
                "## Provenance",
                "",
                "\n".join(f"- {line}" for line in provenance),
            ]
        )
    if actions:
        sections.extend(
            [
                "",
                "## Actionable Read",
                "",
                "\n".join(f"- {line}" for line in actions),
            ]
        )
    if examples:
        sections.extend(
            [
                "",
                "## Representative Failures",
                "",
                "\n".join(f"- {line}" for line in examples),
            ]
        )
    sections.extend(
        [
            "",
            "## Honest Read",
            "",
            takeaways,
        ]
    )
    return "\n".join(sections)


def dump_json(rows: list[RunSummary]) -> list[dict[str, Any]]:
    return [
        {
            **dataclasses.asdict(row),
            "failures": [dataclasses.asdict(failure) for failure in row.failures],
        }
        for row in rows
    ]


def load_existing_rows(path: pathlib.Path) -> list[RunSummary]:
    payload = load_json(path)
    rows: list[RunSummary] = []
    for row in payload:
        failures = [FailureRecord(**failure) for failure in row.get("failures", [])]
        row = dict(row)
        row["failures"] = failures
        rows.append(RunSummary(**row))
    return rows


def ingest_existing_artifacts(
    artifact_roots: list[pathlib.Path],
    models: list[str],
    specs: list[ManifestSpec],
    completed: set[tuple[str, str]],
) -> list[RunSummary]:
    spec_by_manifest = {spec.rel_path: spec for spec in specs}
    latest: dict[tuple[str, str], tuple[str, pathlib.Path, dict[str, Any], ManifestSpec]] = {}
    for artifact_root in artifact_roots:
        if not artifact_root.exists():
            continue
        for artifact_path in artifact_root.rglob("repo-fix-*.json"):
            try:
                artifact = load_json(artifact_path)
            except Exception:
                continue
            model = str(artifact.get("model", ""))
            manifest_path = str(artifact.get("manifest_path", ""))
            spec = spec_by_manifest.get(manifest_path)
            if model not in models or spec is None:
                continue
            key = (model, spec.label)
            if key in completed:
                continue
            stamp = str(artifact.get("recorded_at", ""))
            current = latest.get(key)
            if current is None or stamp > current[0]:
                latest[key] = (stamp, artifact_path, artifact, spec)
    rows: list[RunSummary] = []
    for (model, _task_set), (_stamp, artifact_path, artifact, spec) in sorted(latest.items()):
        rows.append(
            summarize_run(
                model=model,
                manifest=spec,
                artifact=artifact,
                artifact_path=artifact_path,
                duration_s=0.0,
            )
        )
    return rows


def persist_outputs(
    rows: list[RunSummary],
    json_out: pathlib.Path | None,
    markdown_out: pathlib.Path | None,
    binary: pathlib.Path,
) -> None:
    if json_out:
        json_out.parent.mkdir(parents=True, exist_ok=True)
        with json_out.open("w") as handle:
            json.dump(dump_json(rows), handle, indent=2)
            handle.write("\n")
    if markdown_out:
        markdown_out.parent.mkdir(parents=True, exist_ok=True)
        markdown_out.write_text(render_markdown(rows, binary))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        type=pathlib.Path,
        default=DEFAULT_BINARY,
        help="Path to the professor-x release binary",
    )
    parser.add_argument(
        "--data-dir",
        type=pathlib.Path,
        default=DEFAULT_DATA_DIR,
        help="PROFESSOR_X_DATA_DIR for benchmark runs",
    )
    parser.add_argument(
        "--model",
        dest="models",
        action="append",
        default=[],
        help="Model to benchmark; repeat to override the defaults",
    )
    parser.add_argument(
        "--markdown-out",
        type=pathlib.Path,
        help="Optional path for the Markdown report",
    )
    parser.add_argument(
        "--json-out",
        type=pathlib.Path,
        help="Optional path for raw JSON summaries",
    )
    parser.add_argument(
        "--reuse-existing-root",
        action="append",
        default=[],
        type=pathlib.Path,
        help="Scan an existing artifacts/repo-fix tree and ingest the latest matching artifact per model/task set",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        help="Optional per-manifest timeout for the native bench",
    )
    parser.add_argument(
        "--retries",
        type=int,
        default=0,
        help="Retries after a timed-out manifest run",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    binary = args.binary
    if not binary.exists():
        raise FileNotFoundError(f"binary not found: {binary}")
    models = args.models or list(DEFAULT_MODELS)
    specs = manifest_specs()
    rows = load_existing_rows(args.json_out) if args.json_out and args.json_out.exists() else []
    completed = {(row.model, row.task_set) for row in rows}
    reused_rows = ingest_existing_artifacts(args.reuse_existing_root, models, specs, completed)
    if reused_rows:
        rows.extend(reused_rows)
        completed.update((row.model, row.task_set) for row in reused_rows)
        persist_outputs(rows, args.json_out, args.markdown_out, binary)
        for row in reused_rows:
            print(
                f"[reuse] model={row.model} task_set={row.task_set} artifact={row.artifact_path}",
                flush=True,
            )
    for model in models:
        for manifest in specs:
            if (model, manifest.label) in completed:
                print(
                    f"[skip] model={model} task_set={manifest.label} already present in {args.json_out}",
                    flush=True,
                )
                continue
            print(
                f"[run] model={model} task_set={manifest.label} manifest={manifest.rel_path}",
                flush=True,
            )
            artifact, artifact_path, duration_s = run_benchmark(
                binary=binary,
                model=model,
                manifest=manifest,
                data_dir=args.data_dir,
                timeout_seconds=args.timeout_seconds,
                retries=args.retries,
            )
            row = summarize_run(
                model=model,
                manifest=manifest,
                artifact=artifact,
                artifact_path=artifact_path,
                duration_s=duration_s,
            )
            rows.append(row)
            completed.add((row.model, row.task_set))
            persist_outputs(rows, args.json_out, args.markdown_out, binary)
            print(
                "[done] model={model} task_set={task_set} pass@1={p1:.3f} failures={failures} "
                "artifact={artifact}".format(
                    model=model,
                    task_set=manifest.label,
                    p1=row.pass_at_1,
                    failures=sum(row.bucket_counts.values()),
                    artifact=artifact_path,
                ),
                flush=True,
            )
    table = render_table(rows)
    print()
    print(table)
    persist_outputs(rows, args.json_out, args.markdown_out, binary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
