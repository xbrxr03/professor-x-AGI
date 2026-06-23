import sys
from parity import is_even, is_odd
try:
    assert is_even(4) and not is_odd(4)
    assert is_odd(3) and not is_even(3)
    assert is_even(0) and not is_odd(0)
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
