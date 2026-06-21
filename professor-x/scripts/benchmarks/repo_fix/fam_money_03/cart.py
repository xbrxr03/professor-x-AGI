from pricing import price


def total(items, rate_bps, pct):
    return sum(price(c, rate_bps, pct) for c in items)
