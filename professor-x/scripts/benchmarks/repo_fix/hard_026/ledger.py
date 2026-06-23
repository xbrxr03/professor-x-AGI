def apply_entry(balance, entry):
    """Apply one ledger entry. entry: {'type': 'credit'|'debit', 'amount': N}."""
    amt = entry["amount"]
    if entry["type"] == "credit":
        return balance + amt
    else:
        return balance + amt
