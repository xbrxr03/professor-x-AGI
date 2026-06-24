from cells import split_cells, join_cells


def load(text):
    lines = [l for l in text.splitlines() if l.strip()]
    header = split_cells(lines[0])
    return [dict(zip(header, split_cells(l))) for l in lines[1:]]


def pluck(table, key):
    return [r[key] for r in table]


def render(table):
    if not table:
        return ''
    header = list(table[0].keys())
    out = []
    for r in table:
        out.append(join_cells([r[h] for h in header]))
    return '\n'.join(out)
