import sys
from running import running_max
try:
    assert running_max([3, 1, 4, 1, 5]) == 5
    assert running_max([-3, -1, -7]) == -1
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
