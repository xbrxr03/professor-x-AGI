import sys

from routes import hops

try:
    adj = {"A": ["B"], "B": ["C"], "C": ["D"], "D": []}
    # A->B->C->D = 3 hops; A->C = 2 hops; same node = 0; unreachable = -1
    assert hops(adj, [("A", "D")]) == [3], hops(adj, [("A", "D")])
    assert hops(adj, [("A", "C")]) == [2]
    assert hops(adj, [("A", "A")]) == [0]
    assert hops(adj, [("D", "A")]) == [-1]
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
