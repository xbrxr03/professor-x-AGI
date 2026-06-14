import sys
from m import clean
try:
    assert clean('  hi  ') == 'hi'
    assert clean('x') == 'x'
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
