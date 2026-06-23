def to_int(s):
    n=0
    for ch in s:
        if ch.isdigit(): n=n*10+int(ch)
    return n
