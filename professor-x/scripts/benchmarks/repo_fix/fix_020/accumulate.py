def collect(item, bucket=[]):
    # Intended: return a NEW list containing just `item` when no bucket is passed.
    # BUG: the mutable default list is shared across calls, so results accumulate.
    bucket.append(item)
    return bucket
