import sys
from bsearch import bsearch
try:
    assert bsearch([1,3,5,7],7)==3
    assert bsearch([1,3,5,7],1)==0
    assert bsearch([1,3,5,7],4)==-1
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
