import sys
from m import power_two
try:
    assert power_two(3) == 9
    assert power_two(4) == 16
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
