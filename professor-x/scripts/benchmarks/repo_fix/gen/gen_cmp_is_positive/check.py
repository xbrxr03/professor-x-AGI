import sys
from m import is_positive
try:
    assert is_positive(5) == True
    assert is_positive(0) == False
    assert is_positive(-1) == False
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
