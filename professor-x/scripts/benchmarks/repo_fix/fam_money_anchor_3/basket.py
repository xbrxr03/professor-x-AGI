from quote import quote_one


def basket_total(items, bps, rate):
    return sum(quote_one(c, bps, rate) for c in items)
