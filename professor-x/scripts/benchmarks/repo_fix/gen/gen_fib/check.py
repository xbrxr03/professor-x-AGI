import sys
from m import fib
try:
    assert fib(0) == 0
    assert fib(1) == 1
    assert fib(7) == 13
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
