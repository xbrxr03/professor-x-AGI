import sys
from m import count_x
try:
    assert count_x([1,2,2,3],2) == 2
    assert count_x([1],9) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
