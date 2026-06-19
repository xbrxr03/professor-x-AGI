import sys
from client import call
try:
    # succeeds only on the 3rd attempt
    assert call(lambda n:(n==3, n), 3)==3
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
