import sys
from m import factorial
try:
    assert factorial(0) == 1
    assert factorial(5) == 120
    print('ok'); sys.exit(0)
except (AssertionError, RecursionError,):
    print('FAIL'); sys.exit(1)
