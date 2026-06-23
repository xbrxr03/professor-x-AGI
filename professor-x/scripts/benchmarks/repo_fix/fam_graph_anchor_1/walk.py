from adjacency import adjacent


def component(g, start):
    seen = set()
    stack = [start]
    while stack:
        u = stack.pop()
        if u in seen:
            continue
        seen.add(u)
        for v in adjacent(g, u):
            if v not in seen:
                stack.append(v)
    return seen


def linked(g, a, b):
    return b in component(g, a)
