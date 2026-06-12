---
name: add-repofix-fixture
description: "Author a new repo-fix benchmark fixture for Professor X correctly. Use when adding/expanding coding tasks for --repo-fix-bench, growing the benchmark, or making it harder/more representative. Enforces the eval-trust rule: a fixture is only valid once you've verified it goes red→green (broken→fixed) with stdlib checks (NOT pytest — pytest is not installed)."
allowed-tools: Bash(*), Read, Write, Edit, Grep, Glob
argument-hint: [bug-type]
---

# Add a repo-fix benchmark fixture (validated red→green)

The first repo-fix run scored a fake 0/4 because the fixtures used `pytest` (not installed) — a
broken ruler. Every new fixture must be mechanism-checked (see `verify-the-ruler`).

## Layout
`professor-x/scripts/benchmarks/repo_fix/<id>/` contains:
- the buggy source file(s) (keep tiny, < 5 files),
- `check.py` — **stdlib-only** assertions, exits 0 on pass / 1 on fail (NO pytest).

Plus an entry in `professor-x/scripts/benchmarks/repo_fix/tasks.json`:
```json
{ "id": "fix_0NN", "category": "repo_fix",
  "setup": "scripts/benchmarks/repo_fix/fix_0NN",
  "description": "In <file>, <function> has <bug>. Fix it so the test passes.",
  "verify_cmd": "python3 check.py", "expect_exit": 0 }
```

## check.py template
```python
import sys
from <module> import <fn>
try:
    assert <fn>(<in>) == <expected>
    print("ok"); sys.exit(0)
except (AssertionError, <ExpectedError>):
    print("FAIL"); sys.exit(1)
```

## Step 0 — MANDATORY validation before adding (the ruler check)
```bash
cd professor-x/scripts/benchmarks/repo_fix
WD=/tmp/v-<id>; rm -rf $WD; cp -r <id> $WD
echo "buggy: $(cd $WD && python3 check.py >/dev/null 2>&1; echo $?)"   # must be 1 (RED)
# apply the obvious fix to $WD/<file> with python3 -c "...replace...", then:
echo "fixed: $(cd $WD && python3 check.py >/dev/null 2>&1; echo $?)"   # must be 0 (GREEN)
rm -rf $WD
```
If buggy≠1 or fixed≠0, the fixture is broken — fix it before adding. (Shell quoting bit this
before: prefer `python3 -c` for edits with spaces over `sed`.)

## Good bug types (varied difficulty)
wrong operator · off-by-one · missing return · unhandled missing key · wrong boolean (and/or) ·
wrong base case (recursion) · multi-file (bug in an imported helper) · class state (accumulator).

## Output
A fixture committed only after `buggy=1, fixed=0` is shown. Then re-run `--repo-fix-bench` to
record the new representative pass@1 (apply `verify-the-ruler`).
