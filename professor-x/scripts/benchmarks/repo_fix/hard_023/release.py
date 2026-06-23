from semver import cmp_version


def latest(versions):
    """Return the highest version string from a non-empty list."""
    best = versions[0]
    for v in versions[1:]:
        if cmp_version(v, best) > 0:
            best = v
    return best
