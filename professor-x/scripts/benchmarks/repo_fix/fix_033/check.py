import sys
from wc import wc
try:
    assert wc("  a   b ")==2
    assert wc("")==0
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
