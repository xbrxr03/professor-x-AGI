import sys
from temp import c_to_f
try:
    assert c_to_f(0)==32
    assert c_to_f(100)==212
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
