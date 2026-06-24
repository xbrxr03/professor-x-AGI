from clock import Clock

class KVStore:
    """In-memory KV store with per-key TTL, driven by a logical Clock."""
    def __init__(self, clock):
        self._clock = clock
        self._data = {}  # key -> (value, expire_at)

    def set(self, key, value, ttl):
        self._data[key] = (value, self._clock.now() + ttl)

    def get(self, key):
        if key not in self._data:
            return None
        value, expire_at = self._data[key]
        if self._clock.now() > expire_at:   # BUG: should be >= (expires AT expire_at)
            del self._data[key]
            return None
        return value
