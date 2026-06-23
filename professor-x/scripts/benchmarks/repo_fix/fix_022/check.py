import sys
from deepmerge import merge
try:
    assert merge({"a":{"x":1}},{"a":{"y":2}})=={"a":{"x":1,"y":2}}
    assert merge({"p":1},{"q":2})=={"p":1,"q":2}
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
