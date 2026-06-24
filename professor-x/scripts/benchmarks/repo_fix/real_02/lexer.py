def tokenize(s):
    """Split an arithmetic string into int and operator tokens."""
    toks, num = [], ""
    for ch in s:
        if ch.isdigit():
            num += ch
        else:
            if num: toks.append(int(num)); num = ""
            if ch in "+-*/": toks.append(ch)
            elif ch == " ": continue
            else: raise ValueError(f"bad char {ch!r}")
    if num: toks.append(int(num))
    return toks
