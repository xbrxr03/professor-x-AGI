import sys
from m import running_max
try:
    assert running_max([3,1,4]) == 4
    assert running_max([-3,-1,-7]) == -1
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
