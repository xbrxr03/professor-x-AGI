import sys
from clock import Clock
from store import KVStore
try:
    c = Clock(); s = KVStore(c)
    s.set("a", 1, ttl=3)
    assert s.get("a") == 1
    c.advance(2)                       # t=2 < 3, still valid
    assert s.get("a") == 1
    c.advance(1)                       # t=3 == expire_at -> must be expired
    assert s.get("a") is None, ("expired key still live", s.get("a"))
    s.set("b", 2, ttl=5)
    assert s.get("b") == 2
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
