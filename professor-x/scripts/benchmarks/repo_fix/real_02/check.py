import sys
from evaluator import evaluate
try:
    assert evaluate("2+3*4") == 14, evaluate("2+3*4")
    assert evaluate("10-2*3") == 4, evaluate("10-2*3")
    assert evaluate("2*3+4") == 10
    assert evaluate("100/10/2") == 5
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
