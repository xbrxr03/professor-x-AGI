import sys
from invoice import line_total
try:
    assert line_total(1.0, 3, 0.0825)==3.25  # 3.2475 -> 3.25
    assert line_total(2.0,1,0.0)==2.0
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
