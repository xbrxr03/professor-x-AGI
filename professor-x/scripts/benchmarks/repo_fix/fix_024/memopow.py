_cache={}
def power(base,exp):
    key=base
    if key in _cache: return _cache[key]
    r=base**exp
    _cache[key]=r
    return r
