import sys
from schedule import span_days
try:
    assert span_days((2026,6,1),(2026,6,4))==3
    assert span_days((2026,6,10),(2026,6,10))==0
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
