def matmul(a, b):
    """Multiply a (n x m) by b (m x p) -> (n x p)."""
    n = len(a)
    m = len(a[0])
    p = len(b[0])
    out = [[0] * p for _ in range(n)]
    for i in range(n):
        for j in range(p):
            for k in range(m):
                out[i][j] = a[i][k] * b[k][j]
    return out
