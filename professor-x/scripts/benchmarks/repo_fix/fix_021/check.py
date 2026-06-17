import sys
from csvline import parse_csv_line
try:
    assert parse_csv_line('a,"b,c",d')==["a","b,c","d"]
    assert parse_csv_line("x,y")==["x","y"]
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
