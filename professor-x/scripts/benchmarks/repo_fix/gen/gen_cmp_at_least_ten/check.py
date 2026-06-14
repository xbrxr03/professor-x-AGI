import sys
from m import at_least_ten
try:
    assert at_least_ten(10) == True
    assert at_least_ten(9) == False
    assert at_least_ten(11) == True
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
