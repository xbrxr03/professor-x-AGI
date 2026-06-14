import sys
from m import Acc
try:
    a = Acc(); a.add(3); a.add(4)
    assert a.t == 7
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
