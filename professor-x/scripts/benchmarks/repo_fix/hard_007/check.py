import sys
from stats import variance
try:
    assert variance([2,4,6]) == 8/3*1.0 or abs(variance([2,4,6]) - 2.6666666666666665) < 1e-9
    assert abs(variance([1,1,1]) - 0.0) < 1e-9
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
