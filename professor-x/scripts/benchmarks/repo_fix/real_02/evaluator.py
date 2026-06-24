from lexer import tokenize

# BUG: precedence treats + and * equally (single left-to-right pass), so 2+3*4 -> 20, not 14.
PREC = {"+": 1, "-": 1, "*": 1, "/": 1}   # BUG: * and / should be precedence 2

def _apply(a, op, b):
    return {"+": a + b, "-": a - b, "*": a * b, "/": a // b}[op]

def evaluate(s):
    toks = tokenize(s)
    # shunting-yard to RPN using PREC, then evaluate
    out, ops = [], []
    for t in toks:
        if isinstance(t, int):
            out.append(t)
        else:
            while ops and PREC[ops[-1]] >= PREC[t]:
                out.append(ops.pop())
            ops.append(t)
    while ops:
        out.append(ops.pop())
    st = []
    for t in out:
        if isinstance(t, int): st.append(t)
        else: b = st.pop(); a = st.pop(); st.append(_apply(a, t, b))
    return st[0]
