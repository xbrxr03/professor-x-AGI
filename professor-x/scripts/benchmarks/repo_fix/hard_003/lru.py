class LRU:
    def __init__(self, cap):
        self.cap = cap
        self.d = {}
    def get(self, k):
        if k not in self.d: return None
        v = self.d.pop(k); self.d[k] = v
        return v
    def put(self, k, v):
        if k in self.d: self.d.pop(k)
        self.d[k] = v
        if len(self.d) > self.cap:
            self.d.popitem()
