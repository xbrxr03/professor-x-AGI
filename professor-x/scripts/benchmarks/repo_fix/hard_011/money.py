def round_cents(x):
    # BUG: int() truncates toward zero; should round to nearest cent
    return int(x*100)/100
