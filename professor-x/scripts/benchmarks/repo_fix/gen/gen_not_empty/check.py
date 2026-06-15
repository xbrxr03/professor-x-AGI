import sys
from m import is_empty
try:
    assert is_empty([]) == True
    assert is_empty([1]) == False
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
