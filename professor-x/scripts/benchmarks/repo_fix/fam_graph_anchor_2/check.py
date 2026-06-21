import sys
from adjacency import link, adjacent, valence
from walk import component, linked
try:
    g = {}
    link(g, 1, 2); link(g, 2, 3); link(g, 4, 5)
    assert adjacent(g, 2) == {1, 3}, adjacent(g, 2)
    assert valence(g, 2) == 2
    assert valence(g, 1) == 1
    assert component(g, 1) == {1, 2, 3}, component(g, 1)
    assert linked(g, 1, 3) is True
    assert linked(g, 1, 5) is False
    print("ok"); sys.exit(0)
except (AssertionError, KeyError) as e:
    print("FAIL", e); sys.exit(1)
