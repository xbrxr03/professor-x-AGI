import sys
from m import swap
try:
    assert swap((1,2)) == (2, 1)
    assert swap(('a','b')) == ('b', 'a')
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
