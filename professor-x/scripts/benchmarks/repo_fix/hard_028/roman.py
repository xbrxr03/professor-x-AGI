VAL = [
    (1000, "M"),
    (500, "D"),
    (100, "C"),
    (50, "L"),
    (10, "X"),
    (5, "V"),
    (1, "I"),
]


def to_roman(n):
    """Convert a positive integer to a Roman numeral."""
    out = []
    for v, s in VAL:
        while n >= v:
            out.append(s)
            n -= v
    return "".join(out)
