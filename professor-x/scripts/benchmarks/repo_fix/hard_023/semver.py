def parse(v):
    return [int(x) for x in v.split(".")]


def cmp_version(a, b):
    """Return -1 if a < b, 0 if equal, 1 if a > b (semantic version order)."""
    pa, pb = parse(a), parse(b)
    for x, y in zip(pa, pb):
        if x < y:
            return -1
        if x > y:
            return 1
    return 0
