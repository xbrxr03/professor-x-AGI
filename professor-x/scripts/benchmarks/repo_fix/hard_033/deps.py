def indegrees(edges, nodes):
    deg = {n: 0 for n in nodes}
    for _a, b in edges:
        deg[b] += 1
    return deg
