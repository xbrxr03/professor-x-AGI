import sys
from m import clamp
try:
    assert clamp(5,0,10) == 5
    assert clamp(15,0,10) == 10
    assert clamp(-2,0,10) == 0
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
