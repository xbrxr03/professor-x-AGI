import sys
from rle import encode, decode
try:
    assert encode("aaabbc") == [("a",3),("b",2),("c",1)], encode("aaabbc")
    assert encode("x") == [("x",1)]
    for s in ["aaabbc", "x", "zzz", "abc"]:
        assert decode(encode(s)) == s, s
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
