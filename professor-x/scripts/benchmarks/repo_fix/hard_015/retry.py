def run(fn, max_attempts):
    attempts=0; last=None
    # BUG: range(max_attempts-1) -> does one fewer attempt than asked
    for _ in range(max_attempts-1):
        attempts+=1
        ok,val=fn(attempts)
        if ok: return val
        last=val
    return last
