#!/usr/bin/env python3
"""Generate a DIVERSE corpus of repo-fix fixtures for the distillation flywheel (Lever 1).

Each fixture is a self-contained mini-repo with a planted bug + a stdlib `check.py`. The
agent fixing it (red->green) yields a TEST-VERIFIED solving trajectory = gold-standard SFT
data. This generator scales diversity beyond the hand-written 14: parametrized bug templates
× variations, each VALIDATED red->green before it's kept.

Writes fixtures to scripts/benchmarks/repo_fix/gen/<id>/ and a corpus manifest at
scripts/benchmarks/repo_fix/tasks_corpus.json (the 14 curated + all generated), which the
collection bench reads via REPO_FIX_TASKS.

Usage:  python3 distill/gen_fixtures.py [--per-template 6]
"""
import argparse, json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # professor-x/
GEN_DIR = os.path.join(ROOT, "scripts/benchmarks/repo_fix/gen")
BASE_TASKS = os.path.join(ROOT, "scripts/benchmarks/repo_fix/tasks.json")
CORPUS_TASKS = os.path.join(ROOT, "scripts/benchmarks/repo_fix/tasks_corpus.json")


def check_src(module, fn, cases, exc="AssertionError"):
    asserts = "\n    ".join(f"assert {fn}({a}) == {b!r}" for a, b in cases)
    return (f"import sys\nfrom {module} import {fn}\ntry:\n    {asserts}\n"
            f"    print('ok'); sys.exit(0)\nexcept ({exc},):\n    print('FAIL'); sys.exit(1)\n")


# Each template: (label, module, fn, buggy_body, fixed_body, cases, extra_exc, description)
def templates():
    T = []
    # 1. wrong arithmetic operator
    for fn, op_bad, op_ok, cases in [
        ("add", "a - b", "a + b", [("2,3", 5), ("0,0", 0)]),
        ("scale", "x + 2", "x * 2", [("3", 6), ("0", 0)]),
        ("diff", "a + b", "a - b", [("5,3", 2), ("3,3", 0)]),
        ("power_two", "x + x", "x * x", [("3", 9), ("4", 16)]),
    ]:
        T.append((f"op_{fn}", "m", fn, f"def {fn}({'a, b' if ',' in cases[0][0] else 'x'}):\n    return {op_bad}\n",
                  f"def {fn}({'a, b' if ',' in cases[0][0] else 'x'}):\n    return {op_ok}\n", cases, "AssertionError",
                  f"In m.py, {fn} uses the wrong arithmetic operator. Fix it so the tests pass."))
    # 2. off-by-one / wrong index
    T.append(("idx_last", "m", "last", "def last(xs):\n    return xs[len(xs)]\n",
              "def last(xs):\n    return xs[-1]\n", [("[1,2,3]", 3), ("['a','b']", 'b')], "AssertionError, IndexError",
              "In m.py, last(xs) is off by one (xs[len(xs)]). Return the last element."))
    T.append(("idx_first", "m", "first", "def first(xs):\n    return xs[1]\n",
              "def first(xs):\n    return xs[0]\n", [("[5,6,7]", 5), ("[9]", 9)], "AssertionError, IndexError",
              "In m.py, first(xs) returns the second element, not the first. Fix it."))
    # 3. missing return
    T.append(("noret", "m", "double", "def double(x):\n    x * 2\n",
              "def double(x):\n    return x * 2\n", [("4", 8), ("0", 0)], "AssertionError",
              "In m.py, double(x) computes but never returns. Add the missing return."))
    # 4. wrong comparison
    for fn, bad, ok, cases in [
        ("is_positive", "x >= 0", "x > 0", [("5", True), ("0", False), ("-1", False)]),
        ("at_least_ten", "x > 10", "x >= 10", [("10", True), ("9", False), ("11", True)]),
    ]:
        T.append((f"cmp_{fn}", "m", fn, f"def {fn}(x):\n    return {bad}\n",
                  f"def {fn}(x):\n    return {ok}\n", cases, "AssertionError",
                  f"In m.py, {fn} uses the wrong comparison operator. Fix it."))
    # 5. wrong boolean
    T.append(("bool_range", "m", "in_range", "def in_range(x, lo, hi):\n    return x >= lo or x <= hi\n",
              "def in_range(x, lo, hi):\n    return x >= lo and x <= hi\n",
              [("5,1,10", True), ("15,1,10", False), ("-3,1,10", False)], "AssertionError",
              "In m.py, in_range uses OR so it's always true. Require x within [lo, hi]."))
    # 6. wrong init / accumulator
    T.append(("acc_overwrite", "m", "total", None, None, None, None, None))  # special, below
    T.append(("max_init", "m", "running_max", "def running_max(xs):\n    m = 0\n    for x in xs:\n        if x > m:\n            m = x\n    return m\n",
              "def running_max(xs):\n    m = xs[0]\n    for x in xs:\n        if x > m:\n            m = x\n    return m\n",
              [("[3,1,4]", 4), ("[-3,-1,-7]", -1)], "AssertionError",
              "In m.py, running_max inits m=0 so it fails on all-negative input. Fix the init."))
    # 7. recursion base case
    T.append(("rec_fact", "m", "factorial", "def factorial(n):\n    if n == 1:\n        return 1\n    return n * factorial(n - 1)\n",
              "def factorial(n):\n    if n <= 1:\n        return 1\n    return n * factorial(n - 1)\n",
              [("0", 1), ("5", 120)], "AssertionError, RecursionError",
              "In m.py, factorial(0) recurses forever (base case n==1). Fix the base case."))
    # 8. string method
    T.append(("str_rev", "m", "reverse", "def reverse(s):\n    return s\n",
              "def reverse(s):\n    return s[::-1]\n", [("'abc'", 'cba'), ("''", '')], "AssertionError",
              "In m.py, reverse(s) returns the string unchanged. Return it reversed."))
    T.append(("str_upper", "m", "shout", "def shout(s):\n    return s.lower()\n",
              "def shout(s):\n    return s.upper()\n", [("'hi'", 'HI'), ("'A'", 'A')], "AssertionError",
              "In m.py, shout(s) lowercases instead of uppercasing. Fix it."))
    # 9. list op
    T.append(("filter_even", "m", "evens", "def evens(xs):\n    return [x for x in xs if x % 2 == 1]\n",
              "def evens(xs):\n    return [x for x in xs if x % 2 == 0]\n",
              [("[1,2,3,4]", [2, 4]), ("[5,7]", [])], "AssertionError",
              "In m.py, evens(xs) keeps odd numbers. Fix it to keep even numbers."))
    # 10. dict access
    T.append(("dict_get", "m", "get_count", "def get_count(d, k):\n    return d[k]\n",
              "def get_count(d, k):\n    return d.get(k, 0)\n",
              [("{'a':5},'a'", 5), ("{},'z'", 0)], "AssertionError, KeyError",
              "In m.py, get_count raises KeyError for missing keys. Return 0 instead."))
    # 11. edge case empty
    T.append(("avg_empty", "m", "average", "def average(xs):\n    return sum(xs) / len(xs)\n",
              "def average(xs):\n    return sum(xs) / len(xs) if xs else 0\n",
              [("[2,4,6]", 4), ("[]", 0)], "AssertionError, ZeroDivisionError",
              "In m.py, average(xs) crashes on an empty list. Return 0 for empty input."))
    # 12. integer division vs float
    T.append(("intdiv", "m", "halve", "def halve(n):\n    return n / 2\n",
              "def halve(n):\n    return n // 2\n", [("8", 4), ("7", 3)], "AssertionError",
              "In m.py, halve(n) uses float division but should floor-divide. Fix it."))
    # 13. modulo
    T.append(("mod_even", "m", "is_even", "def is_even(n):\n    return n % 2 == 1\n",
              "def is_even(n):\n    return n % 2 == 0\n", [("4", True), ("3", False)], "AssertionError",
              "In m.py, is_even checks the wrong remainder. Fix it."))
    # 14. string strip/split
    T.append(("strip", "m", "clean", "def clean(s):\n    return s\n",
              "def clean(s):\n    return s.strip()\n", [("'  hi  '", 'hi'), ("'x'", 'x')], "AssertionError",
              "In m.py, clean(s) does not strip surrounding whitespace. Fix it."))
    T.append(("wordcount", "m", "word_count", "def word_count(s):\n    return len(s)\n",
              "def word_count(s):\n    return len(s.split())\n", [("'a b c'", 3), ("'one'", 1)], "AssertionError",
              "In m.py, word_count returns characters, not words. Count words instead."))
    # 15. list build: append vs extend / wrong accumulate
    T.append(("sum_list", "m", "total", "def total(xs):\n    s = 0\n    for x in xs:\n        s = x\n    return s\n",
              "def total(xs):\n    s = 0\n    for x in xs:\n        s += x\n    return s\n",
              [("[1,2,3]", 6), ("[]", 0)], "AssertionError",
              "In m.py, total(xs) overwrites the sum instead of accumulating. Fix it."))
    # 16. min/max default
    T.append(("clamp", "m", "clamp", "def clamp(x, lo, hi):\n    return max(lo, x)\n",
              "def clamp(x, lo, hi):\n    return max(lo, min(x, hi))\n",
              [("5,0,10", 5), ("15,0,10", 10), ("-2,0,10", 0)], "AssertionError",
              "In m.py, clamp ignores the upper bound. Clamp x to [lo, hi]."))
    # 17. swap
    T.append(("swap", "m", "swap", "def swap(p):\n    return (p[0], p[1])\n",
              "def swap(p):\n    return (p[1], p[0])\n", [("(1,2)", (2, 1)), ("('a','b')", ('b', 'a'))], "AssertionError",
              "In m.py, swap((a,b)) does not swap the pair. Fix it."))
    # 18. count occurrences
    T.append(("count", "m", "count_x", "def count_x(xs, t):\n    return len(xs)\n",
              "def count_x(xs, t):\n    return xs.count(t)\n", [("[1,2,2,3],2", 2), ("[1],9", 0)], "AssertionError",
              "In m.py, count_x returns the list length, not occurrences of t. Fix it."))
    # 19. boolean not
    T.append(("not_empty", "m", "is_empty", "def is_empty(xs):\n    return len(xs) > 0\n",
              "def is_empty(xs):\n    return len(xs) == 0\n", [("[]", True), ("[1]", False)], "AssertionError",
              "In m.py, is_empty has inverted logic. Fix it."))
    # 20. fibonacci off-by-one
    T.append(("fib", "m", "fib", "def fib(n):\n    a, b = 0, 1\n    for _ in range(n - 1):\n        a, b = b, a + b\n    return a\n",
              "def fib(n):\n    a, b = 0, 1\n    for _ in range(n):\n        a, b = b, a + b\n    return a\n",
              [("0", 0), ("1", 1), ("7", 13)], "AssertionError",
              "In m.py, fib(n) is off by one in its loop range. Fix it."))
    # 21. palindrome
    T.append(("palindrome", "m", "is_pal", "def is_pal(s):\n    return s == s\n",
              "def is_pal(s):\n    return s == s[::-1]\n", [("'aba'", True), ("'ab'", False)], "AssertionError",
              "In m.py, is_pal always returns True. Check the string against its reverse."))
    # 22. default mutable arg style (return wrong)
    T.append(("dedupe", "m", "dedupe", "def dedupe(xs):\n    return xs\n",
              "def dedupe(xs):\n    out = []\n    for x in xs:\n        if x not in out:\n            out.append(x)\n    return out\n",
              [("[1,1,2,3,3]", [1, 2, 3]), ("[]", [])], "AssertionError",
              "In m.py, dedupe(xs) does not remove duplicates. Fix it (preserve order)."))
    return T


def validate(workdir, buggy, fixed, check):
    """Return True iff buggy check fails (1) and fixed check passes (0)."""
    def run(src):
        for f in os.listdir(workdir):
            if f == "__pycache__":
                shutil.rmtree(os.path.join(workdir, f), ignore_errors=True)
        open(os.path.join(workdir, "m.py"), "w").write(src)
        return subprocess.run([sys.executable, "check.py"], cwd=workdir,
                              capture_output=True).returncode
    open(os.path.join(workdir, "check.py"), "w").write(check)
    red = run(buggy)
    green = run(fixed)
    return red == 1 and green == 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--per-template", type=int, default=1, help="(reserved) variations per template")
    ap.parse_args()
    shutil.rmtree(GEN_DIR, ignore_errors=True)
    os.makedirs(GEN_DIR, exist_ok=True)

    tasks = json.load(open(BASE_TASKS))["tasks"]  # start from the curated 14
    kept, skipped = 0, 0
    for label, module, fn, buggy, fixed, cases, exc, desc in templates():
        if buggy is None:  # special: accumulator
            buggy = "class Acc:\n    def __init__(self):\n        self.t = 0\n    def add(self, x):\n        self.t = x\n        return self.t\n"
            fixed = "class Acc:\n    def __init__(self):\n        self.t = 0\n    def add(self, x):\n        self.t += x\n        return self.t\n"
            check = ("import sys\nfrom m import Acc\ntry:\n    a = Acc(); a.add(3); a.add(4)\n"
                     "    assert a.t == 7\n    print('ok'); sys.exit(0)\nexcept AssertionError:\n    print('FAIL'); sys.exit(1)\n")
            desc = "In m.py, Acc.add overwrites the total instead of accumulating. Fix it."
        else:
            check = check_src(module, fn, cases, exc)
        fid = f"gen_{label}"
        with tempfile.TemporaryDirectory() as td:
            if not validate(td, buggy, fixed, check):
                print(f"  SKIP {fid} (failed red->green validation)")
                skipped += 1
                continue
        d = os.path.join(GEN_DIR, fid)
        os.makedirs(d, exist_ok=True)
        open(os.path.join(d, "m.py"), "w").write(buggy)
        open(os.path.join(d, "check.py"), "w").write(check)
        tasks.append({
            "id": fid, "category": "repo_fix",
            "setup": f"scripts/benchmarks/repo_fix/gen/{fid}",
            "description": desc, "verify_cmd": "python3 check.py", "expect_exit": 0,
        })
        kept += 1
    json.dump({"tasks": tasks}, open(CORPUS_TASKS, "w"), indent=2)
    print(f"\nkept {kept} generated fixtures (+ {len(tasks)-kept} curated) -> {len(tasks)} total")
    print(f"skipped {skipped}; corpus manifest: {CORPUS_TASKS}")


if __name__ == "__main__":
    main()
