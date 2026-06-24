def apply_levy(amount, bps):
    # rate in basis points: 10000 bps = 100%
    return amount * bps // 10000
