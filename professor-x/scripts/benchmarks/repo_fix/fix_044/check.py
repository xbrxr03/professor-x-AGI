import sys
from rmprefix import remove_prefix
try:
    assert remove_prefix("ununited","un")=="united"
    assert remove_prefix("happy","un")=="happy"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
