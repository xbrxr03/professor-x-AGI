class TokenBucket:
    """Refills `refill` tokens per tick, capped at capacity."""
    def __init__(self, capacity, refill):
        self.cap = capacity; self.refill = refill
        self.tokens = capacity
    def advance(self, ticks=1):
        self.tokens = min(self.cap, self.tokens + self.refill)   # BUG: ignores `ticks` (should be + ticks*self.refill)
    def allow(self, cost=1):
        if self.tokens >= cost:
            self.tokens -= cost; return True
        return False
