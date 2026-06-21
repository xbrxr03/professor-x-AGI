def intersects(a, b):
    return a[0] <= b[1] and b[0] <= a[1]


def fuse(a, b):
    return (min(a[0], b[0]), max(a[1], b[1]))
