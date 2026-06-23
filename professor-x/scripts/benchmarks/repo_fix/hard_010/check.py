import sys
from door import Door
try:
    d=Door(); d.do('open'); assert d.state=='open'
    d.do('close'); assert d.state=='closed'
    d.do('lock'); assert d.state=='locked'
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
