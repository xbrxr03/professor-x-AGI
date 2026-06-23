import sys
from row import parse_row, to_row
from records import parse, select, dump
try:
    assert parse_row(" a, b ,c ") == ["a","b","c"], parse_row(" a, b ,c ")
    assert to_row([1,2,3]) == "1,2,3"
    recs = parse("name,age\nalice,30\nbob,25")
    assert recs == [{"name":"alice","age":"30"},{"name":"bob","age":"25"}], recs
    assert select(recs, "name") == ["alice","bob"]
    d = dump(recs)
    assert d == "name,age\nalice,30\nbob,25", repr(d)
    assert parse(dump(recs)) == recs  # round-trip
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
