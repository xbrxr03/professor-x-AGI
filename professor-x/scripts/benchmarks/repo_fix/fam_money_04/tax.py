def add_tax(cents, rate_bps):
    # rate in basis points: 10000 bps = 100%
    return cents * rate_bps // 10000
