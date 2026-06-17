def transpose(m):
    rows=len(m); cols=len(m[0])
    return [[m[r][c] for c in range(cols)] for r in range(rows)]
