def parse_row(line):
    return line.split(',')


def to_row(cells):
    return ','.join(str(c) for c in cells)
