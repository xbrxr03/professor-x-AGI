import sys
from routes import reachable
try:
    g={'a':['b','c'],'b':['d'],'c':['d'],'d':[]}
    assert reachable(g,'a')==['a','b','c','d']
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
