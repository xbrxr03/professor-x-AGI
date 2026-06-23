import sys
from interleave import interleave
try:
    assert interleave([1,2,3],[4,5,6])==[1,4,2,5,3,6]
    assert interleave([],[])==[]
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
