import sys
from cart import final
try:
    # 100 -A-> 90 -B-> 72.0  (sequential), NOT 100*(1-0.3)=70
    assert final(100,['A','B'])==72.0
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)
