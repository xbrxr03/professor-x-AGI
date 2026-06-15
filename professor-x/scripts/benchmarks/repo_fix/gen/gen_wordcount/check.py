import sys
from m import word_count
try:
    assert word_count('a b c') == 3
    assert word_count('one') == 1
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
