from matrix import matmul


def apply(matrix, vector):
    """Apply `matrix` to `vector` (as a column) and return the result vector."""
    col = [[v] for v in vector]
    res = matmul(matrix, col)
    return [row[0] for row in res]
