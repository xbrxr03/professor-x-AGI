from collections import deque

from deps import indegrees


def toposort(nodes, edges):
    """Kahn's algorithm: return a topological order of nodes."""
    deg = indegrees(edges, nodes)
    adj = {n: [] for n in nodes}
    for a, b in edges:
        adj[a].append(b)
    q = deque([n for n in nodes if deg[n] == 0])
    out = []
    while q:
        n = q.popleft()
        out.append(n)
        for m in adj[n]:
            deg[m] -= 1
    return out
