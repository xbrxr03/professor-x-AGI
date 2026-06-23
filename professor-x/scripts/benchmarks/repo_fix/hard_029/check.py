import sys

from compress import compress_all

try:
    assert compress_all(["aaab"]) == ["a3b1"], compress_all(["aaab"])
    assert compress_all(["abc"]) == ["a1b1c1"]
    assert compress_all(["x"]) == ["x1"]
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
