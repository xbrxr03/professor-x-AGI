import sys
from counts import get_count
try:
    assert get_count({"a": 5}, "a") == 5
    assert get_count({}, "missing") == 0
    print("ok"); sys.exit(0)
except (AssertionError, KeyError):
    print("FAIL"); sys.exit(1)
