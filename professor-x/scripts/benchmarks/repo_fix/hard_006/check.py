import sys
from settings import resolve
try:
    d = {'db': {'host': 'x', 'port': 5432}, 'debug': False}
    o = {'db': {'port': 6000}}
    r = resolve(d, o)
    assert r == {'db': {'host': 'x', 'port': 6000}, 'debug': False}
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
