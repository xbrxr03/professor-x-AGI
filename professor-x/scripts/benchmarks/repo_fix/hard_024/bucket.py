class TokenBucket:
    """Deterministic token bucket; `now` is supplied by the caller (seconds)."""

    def __init__(self, capacity, refill_per_sec):
        self.capacity = capacity
        self.refill_per_sec = refill_per_sec
        self.tokens = float(capacity)
        self.last = 0.0

    def _refill(self, now):
        elapsed = now - self.last
        self.tokens = min(self.capacity, self.tokens + self.capacity * elapsed)
        self.last = now

    def allow(self, now, cost=1):
        self._refill(now)
        if self.tokens >= cost:
            self.tokens -= cost
            return True
        return False
