import sys
from flatten import flatten
try:
    assert flatten([1, [2, [3, 4]], 5]) == [1, 2, 3, 4, 5]
    assert flatten([]) == []
    assert flatten([[1], [2], [3]]) == [1, 2, 3]
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
