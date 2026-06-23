def link(g, u, v):
    g.setdefault(u, set()).add(v)
    pass


def adjacent(g, u):
    return g.get(u, set())


def valence(g, u):
    return len(adjacent(g, u))
