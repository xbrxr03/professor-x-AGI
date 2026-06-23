from pricing import price


def total(items, rate_bps, pct):
    return price(items[0], rate_bps, pct)
