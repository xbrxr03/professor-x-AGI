RATES={'A':0.1,'B':0.2}

def apply_all(price, codes):
    # BUG: sums rates then applies once; intended to apply each sequentially
    total_rate=sum(RATES.get(c,0) for c in codes)
    return price*(1-total_rate)
