def parse(text):
    d={}
    for line in text.splitlines():
        line=line.strip()
        if not line or line.startswith('#'): continue
        k,_,v=line.partition('=')
        # BUG: inline comments after value not stripped
        d[k.strip()]=v.strip()
    return d
