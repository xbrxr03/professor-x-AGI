import sys
from transitions import SHUT, AJAR, step
from fsm import execute, tally_open
try:
    assert step(SHUT, "coin") == AJAR
    assert step(AJAR, "push") == SHUT
    assert step(SHUT, "push") == SHUT
    assert execute(["coin","push"]) == SHUT
    assert execute(["coin","coin"]) == AJAR
    assert execute([]) == SHUT
    assert tally_open(["coin","push","coin"]) == 2, tally_open(["coin","push","coin"])
    assert tally_open(["push","push"]) == 0
    print("ok"); sys.exit(0)
except (AssertionError, KeyError) as e:
    print("FAIL", e); sys.exit(1)
