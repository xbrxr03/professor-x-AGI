import sys
from lru import LRU
try:
    c = LRU(2)
    c.put('a',1); c.put('b',2); c.get('a'); c.put('c',3)
    # 'b' is LRU and must be evicted; 'a' and 'c' remain
    assert c.get('b') is None
    assert c.get('a') == 1 and c.get('c') == 3
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
