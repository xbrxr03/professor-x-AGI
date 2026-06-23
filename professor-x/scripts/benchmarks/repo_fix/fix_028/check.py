import sys
from transpose import transpose
try:
    assert transpose([[1,2],[3,4]])==[[1,3],[2,4]]
    assert transpose([[1,2,3]])==[[1],[2],[3]]
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
