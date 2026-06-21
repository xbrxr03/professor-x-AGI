from levy import apply_levy
from markdown import apply_markdown


def quote_one(amount, bps, rate):
    # markdown first, then levy
    return apply_markdown(amount, rate)
