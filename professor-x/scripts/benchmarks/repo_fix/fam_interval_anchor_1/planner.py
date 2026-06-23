from spans import intersects, fuse


def consolidate(items):
    items = sorted(items)
    out = [items[0]]
    for iv in items[1:]:
        if intersects(out[-1], iv) or out[-1][1] == iv[0]:
            out[-1] = fuse(out[-1], iv)
        else:
            out.append(iv)
    return out


def span_total(items):
    return sum(e - s for s, e in consolidate(items))
