from quote import quote_one


def basket_total(items, bps, rate):
    return quote_one(items[0], bps, rate)
