import sys
from memopow import power
try:
    assert power(2,3)==8
    assert power(2,4)==16
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
