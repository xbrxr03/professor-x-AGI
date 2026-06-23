import sys
from search import matches
try:
    assert matches('The Cat sat','cat') is True
    assert matches('dog','cat') is False
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
