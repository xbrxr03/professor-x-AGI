def parse_row(line):
    return [c.strip() for c in line.split(',')]


def to_row(cells):
    return ','.join(str(c) for c in cells)
