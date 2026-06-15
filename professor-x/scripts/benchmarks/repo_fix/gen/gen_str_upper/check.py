import sys
from m import shout
try:
    assert shout('hi') == 'HI'
    assert shout('A') == 'A'
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
