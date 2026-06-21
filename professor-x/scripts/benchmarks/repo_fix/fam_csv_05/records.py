from row import parse_row, to_row


def parse(text):
    lines = [l for l in text.splitlines() if l.strip()]
    header = parse_row(lines[0])
    return [dict(zip(header, parse_row(l))) for l in lines[1:]]


def select(records, key):
    return [r[key] for r in records]


def dump(records):
    if not records:
        return ''
    header = list(records[0].keys())
    out = []
    for r in records:
        out.append(to_row([r[h] for h in header]))
    return '\n'.join(out)
