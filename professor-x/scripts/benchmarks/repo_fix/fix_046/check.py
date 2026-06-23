import sys
from median import median
try:
    assert median([3,1,2])==2
    assert median([4,1,2,3])==2.5
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
