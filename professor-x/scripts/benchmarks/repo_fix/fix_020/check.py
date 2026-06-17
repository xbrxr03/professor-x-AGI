import sys
from accumulate import collect
try:
    assert collect(1) == [1]
    assert collect(2) == [2]   # buggy returns [1, 2] because the default list is shared
    assert collect(3, [9]) == [9, 3]
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
