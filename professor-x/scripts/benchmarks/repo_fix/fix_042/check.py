import sys
from mymax import my_max
try:
    assert my_max([3,7,2])==7
    assert my_max([-3,-1,-2])==-1
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
