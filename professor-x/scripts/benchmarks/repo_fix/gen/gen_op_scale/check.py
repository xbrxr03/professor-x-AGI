import sys
from m import scale
try:
    assert scale(3) == 6
    assert scale(0) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
