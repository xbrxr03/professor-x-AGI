def overlaps(a, b):
    return a[0] < b[1] and b[0] < a[1]


def merge_pair(a, b):
    return (b[0], max(a[1], b[1]))
