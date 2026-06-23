import sys

from freq import counts

try:
    # commas must act as separators so "a," and "a" are the same token
    c = counts("a, a, b")
    assert c.get("a") == 2, c
    assert c.get("b") == 1, c
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
