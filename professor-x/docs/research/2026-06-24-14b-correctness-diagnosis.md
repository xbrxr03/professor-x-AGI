# 14B correctness diagnosis: the frontier-feel gap is EDIT-MECHANICS, not reasoning (2026-06-24)

Frontier-Feel scorecard: 14B index 0.928, blocked only by correctness (0.533 < 0.60 bar). Diagnosed
14B's 13 wrong-edit failures on the hard set (made an edit, still red) via diff inspection. The gap is
NOT model reasoning — it's EDIT QUALITY + missing verifier-feedback retry. This is the harness leg
(Codex's lane); Claude diagnoses, Codex fixes `src/`.

## Failure modes (13 wrong-edits)
| mode | tasks | what happened |
|---|---|---|
| **malformed edit** (breaks file) | hard_026, hard_018, hard_006 | indentation error; duplicate `import heapq as pq`; duplicate `return out` |
| **no-op / cosmetic** | hard_029, hard_019, hard_014 | `""`→`''`; sum()→equivalent loop; whitespace-only change |
| **comment-not-code** | hard_009, hard_014 | edited/added a `# FIX:` comment instead of the actual code |
| **hallucinated import** | hard_013 | invented `from dateutil import days_between` (no such module) |
| **wrong-value guess** | hard_005, hard_028 | PAGE_SIZE 3→5 (guessed); changed a data tuple to a wrong value |
| **incomplete rewrite** | hard_012 | rewrote BFS, added `visited=set()` but never uses it |

## The harness levers that close 0.53 → 0.60+ (FOR CODEX, `src/`)
1. **Stricter pre-apply lint + reject malformed edits** (→ retry, don't apply): `py_compile`/parse the
   post-edit file; reject indentation errors (hard_026), duplicate-line insertions (hard_018, hard_006).
   ~3 tasks. (The editverify lint-gate exists — confirm it runs on the NATIVE path and rejects these.)
2. **No-op / cosmetic-edit detection** (→ force a substantive retry): if an edit APPLIES but the verifier
   is STILL red AND the diff is cosmetic/comment/whitespace-only, treat as "no progress" and re-prompt for
   a DIFFERENT edit. Catches hard_029/019/014/009. ~4 tasks.
3. **Verifier-feedback retry (RLEF)**: on a failed post-edit verify, re-prompt with the actual check.py
   output ("your edit did not fix it; the test still fails: <stderr>") and require a different change.
   This is the single biggest lever — most of the 13 would get a 2nd shot instead of a forfeit.
4. **Hallucinated-import guard**: if an edit adds `import X` for a module not in the workspace/stdlib,
   reject (hard_013). Cheap.

## Honest scope
- 14B's REASONING is largely fine (it finds the right region; made_edit 0.967). The losses are
  mechanics + no retry. So correctness is a HARNESS win, not a model win — exactly the "harness is the
  intelligence" thesis, now localized to 4 concrete levers.
- K=1 single run; confirm modes hold on K=3 + the real-feel tier (queued).
