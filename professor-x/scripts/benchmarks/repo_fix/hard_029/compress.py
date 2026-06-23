from rle import encode


def compress_all(words):
    """Run-length encode each word."""
    return [encode(w) for w in words]
