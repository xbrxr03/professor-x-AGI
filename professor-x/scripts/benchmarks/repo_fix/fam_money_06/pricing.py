from tax import add_tax
from discount import apply_discount


def price(cents, rate_bps, pct):
    # discount first, then tax
    return add_tax(apply_discount(cents, pct), rate_bps)
