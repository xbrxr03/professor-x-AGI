import sys
from states import LOCKED, OPEN, next_state
from machine import run, count_opens
try:
    assert next_state(LOCKED, "coin") == OPEN
    assert next_state(OPEN, "push") == LOCKED
    assert next_state(LOCKED, "push") == LOCKED
    assert run(["coin","push"]) == LOCKED
    assert run(["coin","coin"]) == OPEN
    assert run([]) == LOCKED
    assert count_opens(["coin","push","coin"]) == 2, count_opens(["coin","push","coin"])
    assert count_opens(["push","push"]) == 0
    print("ok"); sys.exit(0)
except (AssertionError, KeyError) as e:
    print("FAIL", e); sys.exit(1)
