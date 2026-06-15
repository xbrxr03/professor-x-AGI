import sys
from m import average
try:
    assert average([2,4,6]) == 4
    assert average([]) == 0
    print('ok'); sys.exit(0)
except (AssertionError, ZeroDivisionError,):
    print('FAIL'); sys.exit(1)
