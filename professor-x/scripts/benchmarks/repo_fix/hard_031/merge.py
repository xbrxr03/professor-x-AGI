def apply_overrides(base, overrides):
    """Return a copy of base updated with overrides (overrides win)."""
    out = dict(base)
    for k, v in overrides.items():
        out[k] = v
    return out
