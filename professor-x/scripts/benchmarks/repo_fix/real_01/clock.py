class Clock:
    """A deterministic monotonic clock (logical ticks)."""
    def __init__(self):
        self._t = 0
    def now(self):
        return self._t
    def advance(self, n=1):
        self._t += n
