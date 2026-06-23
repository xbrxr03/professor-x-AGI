def fill(s, key, val):
    # BUG: replaces only the first occurrence
    i=s.find(key)
    if i<0: return s
    return s[:i]+val+s[i+len(key):]
