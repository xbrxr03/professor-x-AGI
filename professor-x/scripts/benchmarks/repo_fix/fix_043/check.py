import sys
from freq import counts
try:
    assert counts(["a","b","a"])=={"a":2,"b":1}
    assert counts([])=={}
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
