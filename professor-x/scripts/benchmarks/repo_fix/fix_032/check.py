import sys
from titlecase import title
try:
    assert title("hello world")=="Hello World"
    assert title("a b c")=="A B C"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
