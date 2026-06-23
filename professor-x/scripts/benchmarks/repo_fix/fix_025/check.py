import sys
from stack import Stack
try:
    s=Stack()
    for v in [1,2,3]: s.push(v)
    assert s.pop()==3
    assert s.pop()==2
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
