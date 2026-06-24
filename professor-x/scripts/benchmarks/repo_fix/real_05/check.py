import sys
from bucket import TokenBucket
try:
    b = TokenBucket(5, 2)
    for _ in range(5): assert b.allow()      # drain to 0
    assert not b.allow()
    b.advance(2)                              # +4 tokens -> 4
    got = sum(1 for _ in range(4) if b.allow())
    assert got == 4, ("expected 4 refilled tokens", got)
    assert not b.allow()
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
