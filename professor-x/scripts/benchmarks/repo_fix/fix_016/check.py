import sys
from stats import RunningAverage
try:
    r = RunningAverage()
    for v in [2, 4, 6]:
        r.add(v)
    assert r.mean() == 4
    r2 = RunningAverage()
    assert r2.mean() == 0
    print("ok"); sys.exit(0)
except (AssertionError, ZeroDivisionError):
    print("FAIL"); sys.exit(1)
