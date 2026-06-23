import sys

from coverage import total_covered
from intervals import merge

try:
    # a fully-nested interval must not shrink the enclosing one
    assert merge([(1, 5), (2, 3)]) == [(1, 5)], merge([(1, 5), (2, 3)])
    assert total_covered([(1, 5), (2, 3)]) == 4
    assert merge([(1, 3), (2, 4), (6, 8)]) == [(1, 4), (6, 8)]
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
