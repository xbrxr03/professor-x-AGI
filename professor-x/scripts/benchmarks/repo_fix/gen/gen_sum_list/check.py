import sys
from m import total
try:
    assert total([1,2,3]) == 6
    assert total([]) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
