class LRUCache:
    """Least-Recently-Used cache with fixed capacity."""
    def __init__(self, capacity):
        self.cap = capacity
        self._order = []      # least-recent first, most-recent last
        self._data = {}
    def _touch(self, key):
        self._order.remove(key); self._order.append(key)
    def get(self, key):
        if key not in self._data: return None
        self._touch(key); return self._data[key]
    def put(self, key, value):
        if key in self._data:
            self._data[key] = value; self._touch(key); return
        if len(self._data) >= self.cap:
            evict = self._order[-1]   # BUG: evicts MOST-recent; LRU should evict self._order[0]
            self._order.remove(evict); del self._data[evict]
        self._data[key] = value; self._order.append(key)
