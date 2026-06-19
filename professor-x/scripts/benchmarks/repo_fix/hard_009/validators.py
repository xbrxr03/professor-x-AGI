def is_email(s):
    # BUG: requires exactly one '.' after '@' and rejects '+' local parts
    if '@' not in s: return False
    local, _, domain = s.partition('@')
    if '+' in local: return False
    return '.' in domain
