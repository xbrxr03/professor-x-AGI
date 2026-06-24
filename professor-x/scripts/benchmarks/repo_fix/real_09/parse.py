def parse(v):
    """'1.10.0' -> (1, 10, 0)."""
    return tuple(int(p) for p in v.split("."))
