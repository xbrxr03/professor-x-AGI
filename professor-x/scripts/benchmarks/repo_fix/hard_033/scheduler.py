from toposort import toposort


def order(nodes, edges):
    """Return an execution order respecting the dependency edges."""
    return toposort(nodes, edges)
