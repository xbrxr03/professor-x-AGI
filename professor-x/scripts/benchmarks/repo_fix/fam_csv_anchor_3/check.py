import sys
from cells import split_cells, join_cells
from table import load, pluck, render
try:
    assert split_cells(" a, b ,c ") == ["a","b","c"], split_cells(" a, b ,c ")
    assert join_cells([1,2,3]) == "1,2,3"
    recs = load("name,age\nalice,30\nbob,25")
    assert recs == [{"name":"alice","age":"30"},{"name":"bob","age":"25"}], recs
    assert pluck(recs, "name") == ["alice","bob"]
    d = render(recs)
    assert d == "name,age\nalice,30\nbob,25", repr(d)
    assert load(render(recs)) == recs  # round-trip
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
