import sys
from paginate import page
try:
    items = list(range(1, 11))   # 1..10
    assert page(items, 1, 3) == [1, 2, 3], page(items, 1, 3)
    assert page(items, 2, 3) == [4, 5, 6]
    assert page(items, 4, 3) == [10]
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
