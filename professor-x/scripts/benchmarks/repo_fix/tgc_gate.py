#!/usr/bin/env python3
"""TGC trust-gate (Phase 3, Stream D).

Accept a distilled CANDIDATE over the BASELINE only if its gain GENERALIZES to a held-out set of
RENAMED anchors (behavior-preserving alpha-renames the optimizer never saw) — not merely the train
families — AND the train-vs-held-out "Goodhart gap" stays bounded. This is what makes a self-improvement
step trustworthy and ungameable: you cannot overfit a surface you never trained on.

Decision logic is a PURE function (unit-tested, no GPU). The bench-running mode shells out to the
release binary; the final accept/reject needs the GPU and is coordinated AFTER Codex's Stream E
(no two benches at once).

Usage:
  # dry-run the DECISION on injected pass@1 (no GPU) — also used by the self-test:
  tgc_gate.py --dry-run --base-train 0.30 --cand-train 0.55 --base-anchor 0.29 --cand-anchor 0.45
  # full gate (GPU): run K-pass bench on both sets for both models, then decide:
  tgc_gate.py --baseline qwen3:8b-q4_K_M --candidate profx-distilled-p3 \
      --train scripts/benchmarks/repo_fix/tasks_families.json \
      --heldout /tmp/tasks_anchors_all.json --k 3 --gguf distill/out/gguf/distilled-clean-Q4_K_M.gguf
"""
import argparse, json, subprocess, statistics, sys, os

DEFAULT_MDE = 0.10        # min held-out improvement to count as real (coarse on ~14-tasks)
DEFAULT_GAP_BOUND = 0.20  # max acceptable (train - heldout) gap for the candidate (Goodhart guard)


def decide(base_train, cand_train, base_anchor, cand_anchor,
           mde=DEFAULT_MDE, gap_bound=DEFAULT_GAP_BOUND):
    """Pure TGC decision. Returns (accept: bool, info: dict).
    ACCEPT iff (1) held-out anchor pass@1 improves by >= MDE over baseline AND
              (2) the candidate's Goodhart gap (train - heldout) <= gap_bound (gain generalizes)."""
    anchor_delta = cand_anchor - base_anchor
    train_delta = cand_train - base_train
    cand_gap = cand_train - cand_anchor
    base_gap = base_train - base_anchor
    generalizes = anchor_delta >= mde
    gap_ok = cand_gap <= gap_bound
    accept = bool(generalizes and gap_ok)
    reasons = []
    reasons.append(f"held-out delta {anchor_delta:+.3f} {'>=' if generalizes else '<'} MDE {mde}")
    reasons.append(f"candidate Goodhart gap {cand_gap:+.3f} {'<=' if gap_ok else '>'} bound {gap_bound}")
    if not generalizes:
        reasons.append("REJECT: held-out gain below MDE (does not generalize -> likely train overfit)")
    elif not gap_ok:
        reasons.append("REJECT: Goodhart gap too large (train gain does not carry to held-out)")
    else:
        reasons.append("ACCEPT: generalizes to held-out renamed anchors with bounded gap")
    return accept, {
        "base_train": base_train, "cand_train": cand_train, "train_delta": train_delta,
        "base_anchor": base_anchor, "cand_anchor": cand_anchor, "anchor_delta": anchor_delta,
        "candidate_goodhart_gap": cand_gap, "baseline_goodhart_gap": base_gap,
        "mde": mde, "gap_bound": gap_bound, "accept": accept, "reasons": reasons,
    }


def gguf_is_safe(gguf_path):
    """NaN guard (the 2026-06-22 corrupt-GGUF lesson): a clean re-quantize from f16 errors on NaN, so a
    present, non-empty gguf that a quick metadata read accepts is the cheap precondition. Returns
    (ok, note). Full NaN validation happens in Codex's quantize step; here we refuse to gate a missing
    or zero-byte artifact."""
    if not gguf_path:
        return True, "no gguf path supplied (skipped)"
    if not os.path.isfile(gguf_path):
        return False, f"gguf missing: {gguf_path}"
    if os.path.getsize(gguf_path) < 1_000_000:
        return False, f"gguf suspiciously small: {os.path.getsize(gguf_path)} bytes"
    return True, "gguf present"


def bench_pass1(model, tasks, k):
    """Run the native repo-fix bench K times, return mean pass@1. Needs the GPU."""
    vals = []
    for _ in range(k):
        env = dict(os.environ, PROFESSOR_X_NATIVE_TOOLS="1",
                   PROFESSOR_X_DATA_DIR=os.path.expanduser("~/.professor-x"),
                   REPO_FIX_TASKS=tasks)
        out = subprocess.run(["./target/release/professor-x", "--repo-fix-bench", "--model", model],
                             capture_output=True, text=True, env=env, timeout=1800).stdout
        import re
        m = re.search(r"pass@1 = ([0-9.]+)", out)
        if m:
            vals.append(float(m.group(1)))
    return statistics.mean(vals) if vals else float("nan")


def self_test():
    """Reproducible decision-logic test (no GPU). Mirrors the Goodhart cases the gate must catch."""
    cases = [
        # (bt, ct, ba, ca, expect_accept, label)
        (0.30, 0.50, 0.29, 0.45, True,  "generalizes + bounded gap"),
        (0.30, 0.70, 0.29, 0.31, False, "train up, held-out flat (OVERFIT -> Goodhart)"),
        (0.30, 0.90, 0.29, 0.45, False, "held-out up but gap too large"),
        (0.30, 0.30, 0.29, 0.29, False, "no improvement"),
    ]
    ok = True
    for bt, ct, ba, ca, exp, label in cases:
        got, _ = decide(bt, ct, ba, ca)
        status = "ok" if got == exp else "FAIL"
        if got != exp:
            ok = False
        print(f"  [{status}] {label}: accept={got} (expected {exp})")
    print("self-test:", "PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--base-train", type=float); ap.add_argument("--cand-train", type=float)
    ap.add_argument("--base-anchor", type=float); ap.add_argument("--cand-anchor", type=float)
    ap.add_argument("--baseline"); ap.add_argument("--candidate")
    ap.add_argument("--train"); ap.add_argument("--heldout")
    ap.add_argument("--k", type=int, default=3)
    ap.add_argument("--mde", type=float, default=DEFAULT_MDE)
    ap.add_argument("--gap-bound", type=float, default=DEFAULT_GAP_BOUND)
    ap.add_argument("--gguf")
    a = ap.parse_args()

    if a.self_test:
        self_test()
    if a.dry_run:
        accept, info = decide(a.base_train, a.cand_train, a.base_anchor, a.cand_anchor,
                              a.mde, a.gap_bound)
    else:
        safe, note = gguf_is_safe(a.gguf)
        if not safe:
            print(json.dumps({"accept": False, "reason": f"gguf guard: {note}"})); sys.exit(1)
        bt = bench_pass1(a.baseline, a.train, a.k); ct = bench_pass1(a.candidate, a.train, a.k)
        ba = bench_pass1(a.baseline, a.heldout, a.k); ca = bench_pass1(a.candidate, a.heldout, a.k)
        accept, info = decide(bt, ct, ba, ca, a.mde, a.gap_bound)
        info["gguf_note"] = note
    print(json.dumps(info, indent=2))
    sys.exit(0 if accept else 2)


if __name__ == "__main__":
    main()
