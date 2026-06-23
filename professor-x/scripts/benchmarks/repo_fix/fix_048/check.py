import sys
from getpath import get_path
try:
    assert get_path({"a":{"b":5}},"a.b")==5
    assert get_path({"a":{"b":5}},"a.z",0)==0
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
