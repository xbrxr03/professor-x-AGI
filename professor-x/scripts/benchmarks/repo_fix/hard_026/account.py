from ledger import apply_entry


def final_balance(entries, opening=0):
    bal = opening
    for e in entries:
        bal = apply_entry(bal, e)
    return bal
