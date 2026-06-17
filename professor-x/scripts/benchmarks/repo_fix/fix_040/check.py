import sys
from vowels import count_vowels
try:
    assert count_vowels("AEIou")==5
    assert count_vowels("xyz")==0
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
