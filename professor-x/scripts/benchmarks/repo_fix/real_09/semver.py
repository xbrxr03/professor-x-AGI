from parse import parse

def compare(a, b):
    """Return -1/0/1 for a<b / a==b / a>b by semantic version order."""
    pa, pb = a.split("."), b.split(".")   # BUG: string parts compared lexically, not numerically
    if pa < pb: return -1
    if pa > pb: return 1
    return 0
