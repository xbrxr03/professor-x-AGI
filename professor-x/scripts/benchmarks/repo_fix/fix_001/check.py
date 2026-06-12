import sys
from calc import add
try:
    assert add(2, 3) == 5
    assert add(-1, 1) == 0
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
