import sys
from toint import to_int
try:
    assert to_int("42")==42
    assert to_int("-42")==-42
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
