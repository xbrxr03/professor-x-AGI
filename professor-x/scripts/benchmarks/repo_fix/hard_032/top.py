from freq import counts


def top_word(s):
    """Most frequent token; ties broken alphabetically."""
    c = counts(s)
    best = None
    for w in sorted(c):
        if best is None or c[w] > c[best]:
            best = w
    return best
