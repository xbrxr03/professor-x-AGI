import sys
from safediv import safe_div
try:
    assert safe_div(6,3)==2
    assert safe_div(5,0)==0
    print("ok");sys.exit(0)
except (AssertionError,ZeroDivisionError):
    print("FAIL");sys.exit(1)
