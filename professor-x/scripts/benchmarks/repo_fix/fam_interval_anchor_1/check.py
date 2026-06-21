import sys
from spans import intersects, fuse
from planner import consolidate, span_total
try:
    assert intersects((1,5),(3,7)) is True
    assert intersects((1,3),(3,5)) is False
    assert fuse((1,5),(2,9)) == (1,9)
    assert consolidate([(1,3),(2,4),(6,8)]) == [(1,4),(6,8)], consolidate([(1,3),(2,4),(6,8)])
    assert consolidate([(1,5),(2,3)]) == [(1,5)], consolidate([(1,5),(2,3)])
    assert span_total([(1,5),(2,3)]) == 4
    assert span_total([(1,3),(2,4),(6,8)]) == 5
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
