import sys

from api import simulate

try:
    # capacity 5, refill 1 token/sec. Drain 5 at t=0, then one per second.
    events = [(0, 5), (0, 1), (1, 1), (1, 1)]
    got = simulate(events, capacity=5, rate=1)
    assert got == [True, False, True, False], got
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
