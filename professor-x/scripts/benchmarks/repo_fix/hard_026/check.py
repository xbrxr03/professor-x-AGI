import sys

from account import final_balance

try:
    entries = [
        {"type": "credit", "amount": 100},
        {"type": "debit", "amount": 30},
        {"type": "credit", "amount": 10},
    ]
    assert final_balance(entries) == 80, final_balance(entries)
    assert final_balance([{"type": "debit", "amount": 50}], opening=200) == 150
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)
