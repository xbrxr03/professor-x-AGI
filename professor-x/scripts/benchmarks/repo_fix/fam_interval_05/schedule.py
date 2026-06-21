from intervals import overlaps, merge_pair


def merge_all(ivs):
    ivs = sorted(ivs)
    out = [ivs[0]]
    for iv in ivs[1:]:
        if overlaps(out[-1], iv) or out[-1][1] == iv[0]:
            out[-1] = merge_pair(out[-1], iv)
        else:
            out.append(iv)
    return out


def covered(ivs):
    return sum(e - s for s, e in ivs)
