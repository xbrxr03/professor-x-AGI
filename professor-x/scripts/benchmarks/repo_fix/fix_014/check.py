import sys
from ops import add, sub, mul, div
try:
    assert add(2, 2) == 4
    assert sub(5, 3) == 2
    assert mul(3, 4) == 12
    assert div(8, 2) == 4
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
