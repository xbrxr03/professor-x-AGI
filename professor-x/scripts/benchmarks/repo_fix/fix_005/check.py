import sys
from strutil import reverse
try:
    assert reverse("abc") == "cba"
    assert reverse("") == ""
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
