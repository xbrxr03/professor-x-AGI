import sys
from m import is_pal
try:
    assert is_pal('aba') == True
    assert is_pal('ab') == False
    print('ok'); sys.exit(0)
except (AssertionError,):
    print('FAIL'); sys.exit(1)
