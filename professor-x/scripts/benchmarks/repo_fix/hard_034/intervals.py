def merge(ivs):
    """Merge overlapping intervals into a minimal disjoint set."""
    if not ivs:
        return []
    ivs = sorted(ivs)
    out = [list(ivs[0])]
    for s, e in ivs[1:]:
        if s <= out[-1][1]:
            out[-1][1] = e
        else:
            out.append([s, e])
    return [tuple(x) for x in out]
