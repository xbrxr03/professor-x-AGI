import sys
from m import diff
try:
    assert diff(5,3) == 2
    assert diff(3,3) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
