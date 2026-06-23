from bucket import TokenBucket


def simulate(events, capacity, rate):
    """events: list of (time, cost). Return the allow/deny decision for each."""
    tb = TokenBucket(capacity, rate)
    return [tb.allow(t, c) for t, c in events]
