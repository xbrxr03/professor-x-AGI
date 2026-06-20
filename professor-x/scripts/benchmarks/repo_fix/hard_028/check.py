import sys

from numeral import label

try:
    assert label([4]) == ["IV"], label([4])
    assert label([9]) == ["IX"], label([9])
    assert label([1994]) == ["MCMXCIV"], label([1994])
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
