import sys
from m import double
try:
    assert double(4) == 8
    assert double(0) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
