from graph import neighbors


def reachable(adj, start):
    seen = set()
    stack = [start]
    while stack:
        u = stack.pop()
        if u in seen:
            continue
        seen.add(u)
        for v in neighbors(adj, u):
            if v not in seen:
                stack.append(v)
    return seen


def connected(adj, a, b):
    return b in neighbors(adj, a)
