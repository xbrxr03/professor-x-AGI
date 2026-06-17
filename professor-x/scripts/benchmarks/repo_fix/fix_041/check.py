import sys
from revwords import reverse_words
try:
    assert reverse_words("a b c")=="c b a"
    assert reverse_words("hello world")=="world hello"
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
