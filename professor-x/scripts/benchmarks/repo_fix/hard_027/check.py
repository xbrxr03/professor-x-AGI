import sys

from report import column

try:
    rows = ['a,"b,c",d']
    assert column(rows, 1) == ["b,c"], column(rows, 1)
    assert column(rows, 0) == ["a"]
    assert column(rows, 2) == ["d"]
    print("ok")
    sys.exit(0)
except (AssertionError, IndexError) as e:
    print("FAIL", e)
    sys.exit(1)
