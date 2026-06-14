import sys
from m import dedupe
try:
    assert dedupe([1,1,2,3,3]) == [1, 2, 3]
    assert dedupe([]) == []
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
