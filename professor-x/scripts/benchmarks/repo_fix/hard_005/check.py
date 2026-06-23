import sys
from paginate import page
try:
    items = list(range(10))
    assert page(items, 1) == [0,1,2]
    assert page(items, 2) == [3,4,5]
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
