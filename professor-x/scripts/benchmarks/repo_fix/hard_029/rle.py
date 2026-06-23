def encode(s):
    """Run-length encode: 'aaab' -> 'a3b1'."""
    if not s:
        return ""
    out = []
    prev = s[0]
    cnt = 1
    for ch in s[1:]:
        if ch == prev:
            cnt += 1
        else:
            out.append(prev + str(cnt))
            prev = ch
            cnt = 1
    return "".join(out)
