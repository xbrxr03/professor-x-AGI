import sys
from m import get_count
try:
    assert get_count({'a':5},'a') == 5
    assert get_count({},'z') == 0
    print('ok'); sys.exit(0)
except (AssertionError, KeyError,):
    print('FAIL'); sys.exit(1)
