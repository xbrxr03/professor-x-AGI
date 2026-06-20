import sys

from transform import apply

try:
    # [[1,2],[3,4]] applied to [5,6] = [1*5+2*6, 3*5+4*6] = [17, 39]
    assert apply([[1, 2], [3, 4]], [5, 6]) == [17, 39]
    assert apply([[1, 0], [0, 1]], [7, 9]) == [7, 9]
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
