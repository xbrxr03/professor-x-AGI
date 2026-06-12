import sys
from acc import Accumulator
try:
    a = Accumulator()
    a.add(3); a.add(4)
    assert a.total == 7
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
