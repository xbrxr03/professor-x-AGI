import sys
from config import get
try:
    assert get('port = 8080  # default','port')=='8080'
    assert get('host = x','host')=='x'
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
