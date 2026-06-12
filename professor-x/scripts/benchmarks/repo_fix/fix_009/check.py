import sys
from api import make_slug
try:
    assert make_slug("Hello World") == "hello-world"
    assert make_slug("A B C") == "a-b-c"
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
