import sys
from roman import to_roman
try:
    assert to_roman(4)=="IV"
    assert to_roman(9)=="IX"
    assert to_roman(58)=="LVIII"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
