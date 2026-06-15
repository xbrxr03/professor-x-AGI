import sys
from m import halve
try:
    assert halve(8) == 4
    assert halve(7) == 3
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
