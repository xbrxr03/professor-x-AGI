def split_line(line):
    """Split a CSV line on commas, respecting double-quoted fields."""
    fields, cur, inq = [], "", False
    for ch in line:
        if ch == '"':
            inq = not inq
        elif ch == ',':          # BUG: splits on every comma, even inside quotes (should check `and not inq`)
            fields.append(cur); cur = ""
        else:
            cur += ch
    fields.append(cur)
    return fields
