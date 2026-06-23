import sys
from list import pages
try:
    assert pages(10,3)==4
    assert pages(9,3)==3
    assert pages(0,3)==0
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
