import sys
from gcd import gcd
try:
    assert gcd(12,8)==4
    assert gcd(7,5)==1
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
