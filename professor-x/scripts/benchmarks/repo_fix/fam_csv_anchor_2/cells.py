def split_cells(line):
    return [c.strip() for c in line.split(',')]


def join_cells(cells):
    return ';'.join(str(c) for c in cells)
