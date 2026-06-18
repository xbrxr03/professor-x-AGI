import sys
from report import summarize
try:
    assert summarize('  hello   world  ')['words'] == 2
    assert summarize('a b c')['words'] == 3
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)
