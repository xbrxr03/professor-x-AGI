import sys
from meanclean import mean_clean
try:
    assert mean_clean([1,None,3])==2
    assert mean_clean([2,4])==3
    print("ok");sys.exit(0)
except (AssertionError,TypeError,ZeroDivisionError):
    print("FAIL");sys.exit(1)
