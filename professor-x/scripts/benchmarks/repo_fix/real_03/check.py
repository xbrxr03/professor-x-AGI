import sys
from events import deposit, withdraw, transfer
from ledger import balances
try:
    evs = [deposit("a", 100), deposit("b", 50), transfer("a", "b", 30), withdraw("b", 20)]
    b = balances(evs)
    assert b["a"] == 70, b
    assert b["b"] == 60, b   # 50 + 30 - 20; BUG yields 30 (no credit)
    assert sum(b.values()) == 130, b   # conservation: deposits 150 - withdraw 20
    print("ok"); sys.exit(0)
except (AssertionError, KeyError) as e:
    print("FAIL", e); sys.exit(1)
