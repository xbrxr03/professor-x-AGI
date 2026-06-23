import sys
from alleven import all_even
try:
    assert all_even([2,4,6])==True
    assert all_even([2,3])==False
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
