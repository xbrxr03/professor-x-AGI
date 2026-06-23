import sys
from chunk import chunk
try:
    assert chunk([1,2,3,4,5],2)==[[1,2],[3,4],[5]]
    assert chunk([1,2,3],3)==[[1,2,3]]
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
