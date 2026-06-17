import sys
from rle import rle
try:
    assert rle("aaabbc")=="a3b2c1"
    assert rle("a")=="a1"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
