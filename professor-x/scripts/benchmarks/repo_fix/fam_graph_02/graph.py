def add_edge(adj, u, v):
    adj.setdefault(u, set()).add(v)
    adj.setdefault(v, set()).add(u)


def neighbors(adj, u):
    return adj.get(u, set()) | {u}


def degree(adj, u):
    return len(neighbors(adj, u))
