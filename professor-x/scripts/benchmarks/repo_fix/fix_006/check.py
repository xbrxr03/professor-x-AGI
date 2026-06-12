import sys
from nums import evens
try:
    assert evens([1,2,3,4]) == [2,4]
    assert evens([5,7]) == []
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
