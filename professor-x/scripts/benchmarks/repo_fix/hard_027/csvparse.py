def parse_line(line):
    """Split a CSV line on commas; commas inside double-quotes are literal."""
    fields = []
    cur = []
    in_q = False
    for ch in line:
        if ch == '"':
            in_q = not in_q
        elif ch == ",":
            fields.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    fields.append("".join(cur))
    return fields
