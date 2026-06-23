from collections import deque


def shortest(adj, src, dst):
    """Shortest hop distance from src to dst in an unweighted graph, or -1."""
    if src == dst:
        return 0
    seen = {src}
    q = deque([(src, 0)])
    while q:
        node, d = q.popleft()
        for nb in adj.get(node, []):
            if nb == dst:
                return d
            if nb not in seen:
                seen.add(nb)
                q.append((nb, d + 1))
    return -1
