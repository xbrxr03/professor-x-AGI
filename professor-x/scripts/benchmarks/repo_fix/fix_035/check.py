import sys
from pal import is_pal
try:
    assert is_pal("Aba")==True
    assert is_pal("hello")==False
    assert is_pal("Was it a car or a cat I saw")==True
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)
