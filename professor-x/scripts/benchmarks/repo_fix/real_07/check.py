import sys
from scanner import split_line
try:
    assert split_line('a,b,c') == ['a', 'b', 'c']
    assert split_line('a,"b,c",d') == ['a', 'b,c', 'd'], split_line('a,"b,c",d')
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
