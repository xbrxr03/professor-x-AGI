import sys
from seq import last
try:
    assert last([1, 2, 3]) == 3
    assert last(["a", "b"]) == "b"
    print("ok"); sys.exit(0)
except (AssertionError, IndexError):
    print("FAIL"); sys.exit(1)
