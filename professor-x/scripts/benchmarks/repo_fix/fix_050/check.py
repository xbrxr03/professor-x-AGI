import sys
from padleft import pad_left
try:
    assert pad_left("5",3,"0")=="005"
    assert pad_left("42",2,"0")=="42"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
