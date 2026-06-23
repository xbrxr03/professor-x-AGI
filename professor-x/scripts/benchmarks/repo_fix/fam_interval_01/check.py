import sys
from intervals import overlaps, merge_pair
from schedule import merge_all, covered
try:
    assert overlaps((1,5),(3,7)) is True
    assert overlaps((1,3),(3,5)) is False
    assert merge_pair((1,5),(2,9)) == (1,9)
    assert merge_all([(1,3),(2,4),(6,8)]) == [(1,4),(6,8)], merge_all([(1,3),(2,4),(6,8)])
    assert merge_all([(1,5),(2,3)]) == [(1,5)], merge_all([(1,5),(2,3)])
    assert covered([(1,5),(2,3)]) == 4
    assert covered([(1,3),(2,4),(6,8)]) == 5
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
