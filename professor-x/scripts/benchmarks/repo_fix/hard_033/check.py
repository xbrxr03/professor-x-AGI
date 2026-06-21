import sys

from scheduler import order

try:
    nodes = ["a", "b", "c", "d"]
    edges = [("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]
    o = order(nodes, edges)
    assert len(o) == 4, f"expected all 4 nodes, got {o}"
    pos = {n: i for i, n in enumerate(o)}
    for a, b in edges:
        assert pos[a] < pos[b], f"{a} must come before {b}: {o}"
    print("ok")
    sys.exit(0)
except (AssertionError, KeyError, IndexError) as e:
    print("FAIL", e)
    sys.exit(1)
