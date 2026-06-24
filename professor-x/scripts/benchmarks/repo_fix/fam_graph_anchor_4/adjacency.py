def link(g, u, v):
    g.setdefault(u, set()).add(v)
    g.setdefault(v, set()).add(u)


def adjacent(g, u):
    return g.get(u, set())


def valence(g, u):
    return len(adjacent(g, u))
