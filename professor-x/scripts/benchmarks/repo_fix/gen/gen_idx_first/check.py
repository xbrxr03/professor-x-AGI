import sys
from m import first
try:
    assert first([5,6,7]) == 5
    assert first([9]) == 9
    print('ok'); sys.exit(0)
except (AssertionError, IndexError,):
    print('FAIL'); sys.exit(1)
