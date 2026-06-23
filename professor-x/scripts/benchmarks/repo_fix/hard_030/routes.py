from graph_bfs import shortest


def hops(adj, pairs):
    """Return the shortest hop distance for each (src, dst) pair."""
    return [shortest(adj, a, b) for a, b in pairs]
