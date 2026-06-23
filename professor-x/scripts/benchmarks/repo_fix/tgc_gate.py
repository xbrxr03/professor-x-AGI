#!/usr/bin/env python3
"""TGC trust-gate (Phase 3, Stream D) — COLLATERALIZED.

Accept a distilled CANDIDATE over the BASELINE only if its gain GENERALIZES to a held-out set of
RENAMED anchors (behavior-preserving alpha-renames the optimizer never saw) — not merely the train
families — AND the train-vs-held-out "Goodhart gap" stays bounded AND the gain is COLLATERALIZED
(it does not mask per-anchor drawdown: wins are not paid for by silently regressing anchors the
baseline already passed). The collateral factor is the buildable nugget extracted from Codex's
Collateralized-Global-Workspace idea (docs/research/2026-06-23-collateralized-tgc-gate.md): a scalar
mean pass@1 can hide a +3/-3 trade; per-anchor accounting prices that drawdown.

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
DEFAULT_DRAWDOWN_TOL = 0  # max per-anchor regressions (baseline-passed, candidate-failed) allowed


def decide(base_train, cand_train, base_anchor, cand_anchor,
           mde=DEFAULT_MDE, gap_bound=DEFAULT_GAP_BOUND,
           base_anchor_vec=None, cand_anchor_vec=None, drawdown_tol=DEFAULT_DRAWDOWN_TOL):
    """Pure Collateralized-TGC decision. Returns (accept: bool, info: dict).
    ACCEPT iff ALL hold:
      (1) Realized PnL: held-out anchor pass@1 improves by >= MDE over baseline (generalizes),
      (2) Goodhart gap (train - heldout) <= gap_bound (gain carries to held-out),
      (3) Collateral / no-drawdown: per-anchor, the candidate regresses at most `drawdown_tol`
          anchors the baseline passed (aggregate gain is not paid for by hidden per-anchor losses).
    Factor (3) is evaluated ONLY when aligned per-anchor pass vectors are supplied (else N/A —
    backward-compatible scalar gate). Vectors are 0/1 (or pass-rate; >=0.5 = pass), index-aligned."""
    anchor_delta = cand_anchor - base_anchor
    train_delta = cand_train - base_train
    cand_gap = cand_train - cand_anchor
    base_gap = base_train - base_anchor
    generalizes = anchor_delta >= mde
    gap_ok = cand_gap <= gap_bound

    # (3) collateral / no-drawdown — per-anchor regression accounting
    collateral_checked = base_anchor_vec is not None and cand_anchor_vec is not None
    regressions = improvements = None
    collateral_ok = True
    if collateral_checked:
        if len(base_anchor_vec) != len(cand_anchor_vec):
            raise ValueError("anchor vectors must be index-aligned (same length)")
        pairs = [(1 if b >= 0.5 else 0, 1 if c >= 0.5 else 0) for b, c in zip(base_anchor_vec, cand_anchor_vec)]
        regressions = sum(1 for b, c in pairs if b == 1 and c == 0)   # baseline passed, candidate failed
        improvements = sum(1 for b, c in pairs if b == 0 and c == 1)
        collateral_ok = regressions <= drawdown_tol

    accept = bool(generalizes and gap_ok and collateral_ok)
    reasons = [
        f"held-out delta {anchor_delta:+.3f} {'>=' if generalizes else '<'} MDE {mde}",
        f"candidate Goodhart gap {cand_gap:+.3f} {'<=' if gap_ok else '>'} bound {gap_bound}",
    ]
    if collateral_checked:
        reasons.append(f"collateral: {regressions} per-anchor regression(s) "
                       f"{'<=' if collateral_ok else '>'} tol {drawdown_tol} (improvements {improvements})")
    if not generalizes:
        reasons.append("REJECT: held-out gain below MDE (does not generalize -> likely train overfit)")
    elif not gap_ok:
        reasons.append("REJECT: Goodhart gap too large (train gain does not carry to held-out)")
    elif not collateral_ok:
        reasons.append("REJECT: collateral — aggregate held-out gain MASKS per-anchor drawdown "
                       "(wins paid for by silently regressing anchors the baseline passed)")
    else:
        reasons.append("ACCEPT: generalizes to held-out renamed anchors, bounded gap, no per-anchor drawdown")
    return accept, {
        "base_train": base_train, "cand_train": cand_train, "train_delta": train_delta,
        "base_anchor": base_anchor, "cand_anchor": cand_anchor, "anchor_delta": anchor_delta,
        "candidate_goodhart_gap": cand_gap, "baseline_goodhart_gap": base_gap,
        "anchor_regressions": regressions, "anchor_improvements": improvements,
        "drawdown_tol": (drawdown_tol if collateral_checked else None),
        "collateral_checked": collateral_checked, "collateral_ok": collateral_ok,
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
    mean, _ = bench_vec(model, tasks, k)
    return mean


def bench_vec(model, tasks, k):
    """Run the bench K times; return (mean_pass@1, {task_id: pass_rate over K}). Needs the GPU.
    Per-task results are read from the newest repo-fix artifact each run writes (for the collateral
    factor). If artifacts can't be matched, per-task is {} and the gate degrades to scalar-only."""
    import glob, re
    art_glob = os.path.join("artifacts", "repo-fix", "**", "*.json")
    means, per_task_hits = [], {}
    for _ in range(k):
        before = set(glob.glob(art_glob, recursive=True))
        env = dict(os.environ, PROFESSOR_X_NATIVE_TOOLS="1",
                   PROFESSOR_X_DATA_DIR=os.path.expanduser("~/.professor-x"), REPO_FIX_TASKS=tasks)
        out = subprocess.run(["./target/release/professor-x", "--repo-fix-bench", "--model", model],
                             capture_output=True, text=True, env=env, timeout=7200).stdout
        m = re.search(r"pass@1 = ([0-9.]+)", out)
        if m:
            means.append(float(m.group(1)))
        new = sorted(set(glob.glob(art_glob, recursive=True)) - before, key=os.path.getmtime)
        if new:
            try:
                d = json.load(open(new[-1]))
                for t in d.get("tasks", []):
                    per_task_hits.setdefault(t["id"], []).append(1 if t.get("passed") else 0)
            except Exception:
                pass
    mean = statistics.mean(means) if means else float("nan")
    per_task = {tid: sum(v) / len(v) for tid, v in per_task_hits.items()}
    return mean, per_task


def _aligned_vectors(base_per_task, cand_per_task):
    """Intersect task ids and return index-aligned 0/1 majority-pass vectors (base, cand)."""
    ids = sorted(set(base_per_task) & set(cand_per_task))
    b = [1 if base_per_task[i] >= 0.5 else 0 for i in ids]
    c = [1 if cand_per_task[i] >= 0.5 else 0 for i in ids]
    return b, c, ids


def self_test():
    """Reproducible decision-logic test (no GPU). Mirrors the Goodhart + drawdown cases the gate must catch."""
    ok = True

    # --- scalar factors (1)+(2): unchanged behavior, collateral N/A ---
    scalar_cases = [
        # (bt, ct, ba, ca, expect_accept, label)
        (0.30, 0.50, 0.29, 0.45, True,  "generalizes + bounded gap"),
        (0.30, 0.70, 0.29, 0.31, False, "train up, held-out flat (OVERFIT -> Goodhart)"),
        (0.30, 0.90, 0.29, 0.45, False, "held-out up but gap too large"),
        (0.30, 0.30, 0.29, 0.29, False, "no improvement"),
    ]
    for bt, ct, ba, ca, exp, label in scalar_cases:
        got, _ = decide(bt, ct, ba, ca)
        s = "ok" if got == exp else "FAIL"; ok &= got == exp
        print(f"  [{s}] {label}: accept={got} (expected {exp})")

    # --- collateral factor (3): per-anchor drawdown ---
    # clean held-out gain, no drawdown -> ACCEPT
    bvec = [1, 1, 0, 0, 0, 0, 0, 0]   # base 0.250
    cvec = [1, 1, 1, 1, 0, 0, 0, 0]   # cand 0.500, regressions=0
    got, info = decide(0.30, 0.50, 0.25, 0.50, base_anchor_vec=bvec, cand_anchor_vec=cvec)
    exp = True; s = "ok" if got == exp else "FAIL"; ok &= got == exp
    print(f"  [{s}] clean held-out gain, no drawdown: accept={got} (regressions={info['anchor_regressions']}, expected {exp})")

    # THE decisive case: aggregate gain MASKS drawdown.
    # base 0.500 -> cand 0.625 (delta +0.125 >= MDE) so SCALAR-only would ACCEPT,
    # but candidate fails 2 anchors the baseline passed -> COLLATERAL must REJECT.
    bvec = [1, 1, 1, 1, 0, 0, 0, 0]   # base 0.500
    cvec = [0, 0, 1, 1, 1, 1, 1, 0]   # cand 0.625, regressions=2, improvements=3
    got_scalar, _ = decide(0.40, 0.50, 0.500, 0.625)                       # no vectors -> scalar gate
    got_coll, info = decide(0.40, 0.50, 0.500, 0.625,
                            base_anchor_vec=bvec, cand_anchor_vec=cvec)     # collateralized
    s1 = "ok" if got_scalar is True else "FAIL"; ok &= got_scalar is True
    s2 = "ok" if got_coll is False else "FAIL"; ok &= got_coll is False
    print(f"  [{s1}] gains-mask-drawdown / SCALAR gate: accept={got_scalar} (expected True — the blind spot)")
    print(f"  [{s2}] gains-mask-drawdown / COLLATERAL gate: accept={got_coll} "
          f"(regressions={info['anchor_regressions']} > tol 0, expected False)")

    # tolerance honored: same case with drawdown_tol=2 -> collateral passes (then accept)
    got_tol, _ = decide(0.40, 0.50, 0.500, 0.625, base_anchor_vec=bvec, cand_anchor_vec=cvec, drawdown_tol=2)
    s = "ok" if got_tol is True else "FAIL"; ok &= got_tol is True
    print(f"  [{s}] drawdown_tol=2 honored: accept={got_tol} (expected True)")

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
    ap.add_argument("--drawdown-tol", type=int, default=DEFAULT_DRAWDOWN_TOL,
                    help="max per-anchor regressions (baseline-passed -> candidate-failed) allowed")
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
        def log(m): print(m, file=sys.stderr, flush=True)
        # held-out (renamed anchors) FIRST — the decisive generalization signal; capture per-task for collateral
        ba, ba_pt = bench_vec(a.baseline, a.heldout, a.k); log(f"[gate] base_anchor={ba:.3f}")
        ca, ca_pt = bench_vec(a.candidate, a.heldout, a.k)
        log(f"[gate] cand_anchor={ca:.3f}  held-out delta {ca-ba:+.3f} (MDE {a.mde})")
        bvec, cvec, ids = _aligned_vectors(ba_pt, ca_pt)
        if bvec:
            log(f"[gate] per-anchor collateral over {len(ids)} anchors (drawdown_tol={a.drawdown_tol})")
        else:
            log("[gate] per-anchor artifacts unavailable -> collateral N/A (scalar gate)")
        bt = bench_pass1(a.baseline, a.train, a.k); log(f"[gate] base_train={bt:.3f}")
        ct = bench_pass1(a.candidate, a.train, a.k); log(f"[gate] cand_train={ct:.3f}  train delta {ct-bt:+.3f}")
        accept, info = decide(bt, ct, ba, ca, a.mde, a.gap_bound,
                              base_anchor_vec=(bvec or None), cand_anchor_vec=(cvec or None),
                              drawdown_tol=a.drawdown_tol)
        info["gguf_note"] = note
        info["anchor_ids"] = ids
    print(json.dumps(info, indent=2))
    sys.exit(0 if accept else 2)


if __name__ == "__main__":
    main()
