import sys
from m import in_range
try:
    assert in_range(5,1,10) == True
    assert in_range(15,1,10) == False
    assert in_range(-3,1,10) == False
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
