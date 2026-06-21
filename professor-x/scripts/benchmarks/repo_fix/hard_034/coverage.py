from intervals import merge


def total_covered(ivs):
    """Total length covered by the union of the intervals."""
    return sum(e - s for s, e in merge(ivs))
