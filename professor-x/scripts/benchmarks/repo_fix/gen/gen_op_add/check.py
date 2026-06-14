import sys
from m import add
try:
    assert add(2,3) == 5
    assert add(0,0) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
