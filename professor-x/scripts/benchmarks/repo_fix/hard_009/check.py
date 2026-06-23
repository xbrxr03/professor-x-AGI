import sys
from form import register
try:
    assert register('a+tag@x.com')=='ok'
    assert register('a@x.com')=='ok'
    assert register('bad')=='invalid'
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
