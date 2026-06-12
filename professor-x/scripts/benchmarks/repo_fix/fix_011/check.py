import sys
from stats import sum_of_squares
try:
    assert sum_of_squares([1, 2, 3]) == 14
    assert sum_of_squares([]) == 0
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)
