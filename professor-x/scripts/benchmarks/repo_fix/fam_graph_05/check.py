import sys
from graph import add_edge, neighbors, degree
from traverse import reachable, connected
try:
    adj = {}
    add_edge(adj, 1, 2); add_edge(adj, 2, 3); add_edge(adj, 4, 5)
    assert neighbors(adj, 2) == {1, 3}, neighbors(adj, 2)
    assert degree(adj, 2) == 2
    assert degree(adj, 1) == 1
    assert reachable(adj, 1) == {1, 2, 3}, reachable(adj, 1)
    assert connected(adj, 1, 3) is True
    assert connected(adj, 1, 5) is False
    print("ok"); sys.exit(0)
except (AssertionError, KeyError) as e:
    print("FAIL", e); sys.exit(1)
