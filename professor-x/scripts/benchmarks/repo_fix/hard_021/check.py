import sys
from booking import conflicts
try:
    assert conflicts((1,2),(2,3)) is False
    assert conflicts((1,3),(2,4)) is True
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
