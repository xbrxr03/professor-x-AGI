def encode(s):
    """Run-length encode: 'aaabbc' -> [('a',3),('b',2),('c',1)]."""
    out = []
    if not s: return out
    cur, n = s[0], 1
    for ch in s[1:]:
        if ch == cur:
            n += 1
        else:
            out.append((cur, n)); cur, n = ch, 1
    # BUG: the final run (cur, n) is never appended
    return out

def decode(pairs):
    return "".join(ch * n for ch, n in pairs)
