import sys
from cache import LRUCache
try:
    c = LRUCache(2)
    c.put("a", 1); c.put("b", 2)
    assert c.get("a") == 1          # now a is most-recent, b is LRU
    c.put("c", 3)                   # full -> evict LRU = b
    assert c.get("b") is None, "LRU (b) should be evicted"
    assert c.get("a") == 1
    assert c.get("c") == 3
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
