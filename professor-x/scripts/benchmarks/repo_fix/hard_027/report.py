from csvparse import parse_line


def column(rows, idx):
    """Return the idx-th field of each CSV row."""
    return [parse_line(r)[idx] for r in rows]
