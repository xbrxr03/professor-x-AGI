from tok import tokens


def counts(s):
    d = {}
    for t in tokens(s):
        d[t] = d.get(t, 0) + 1
    return d
