def flatten(xs):
    # Intended: recursively flatten arbitrarily nested lists into a flat list.
    out = []
    for x in xs:
        if isinstance(x, list):
            out.append(x)  # BUG: appends the sublist instead of recursing into it.
        else:
            out.append(x)
    return out
