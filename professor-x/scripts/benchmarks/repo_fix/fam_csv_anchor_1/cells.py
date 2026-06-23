def split_cells(line):
    return line.split(',')


def join_cells(cells):
    return ','.join(str(c) for c in cells)
