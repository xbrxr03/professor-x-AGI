def balances(events):
    """Replay an event log into per-account balances."""
    bal = {}
    for e in events:
        kind = e[0]
        if kind == "deposit":
            _, a, amt = e; bal[a] = bal.get(a, 0) + amt
        elif kind == "withdraw":
            _, a, amt = e; bal[a] = bal.get(a, 0) - amt
        elif kind == "transfer":
            _, s, d, amt = e
            bal[s] = bal.get(s, 0) - amt
            # BUG: transfer debits source but never credits destination
    return bal
