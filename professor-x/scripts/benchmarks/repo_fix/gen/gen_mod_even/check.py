import sys
from m import is_even
try:
    assert is_even(4) == True
    assert is_even(3) == False
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
