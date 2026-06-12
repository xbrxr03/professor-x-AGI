import sys
from valid import in_range
try:
    assert in_range(5, 1, 10) is True
    assert in_range(15, 1, 10) is False
    assert in_range(-3, 1, 10) is False
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
