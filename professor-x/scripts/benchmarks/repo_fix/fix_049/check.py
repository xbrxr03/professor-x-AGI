import sys
from sumdigits import sum_digits
try:
    assert sum_digits(123)==6
    assert sum_digits(9)==9
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
