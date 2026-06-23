import sys
from store import is_live
try:
    assert is_live(5,10) is True
    assert is_live(10,10) is True
    assert is_live(11,10) is False
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
