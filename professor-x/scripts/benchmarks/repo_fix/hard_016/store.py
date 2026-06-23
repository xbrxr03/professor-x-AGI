import ttl

def is_live(age, limit):
    return ttl.fresh(age, limit)
