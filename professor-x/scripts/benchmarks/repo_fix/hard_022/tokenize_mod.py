def tokens(s):
    # BUG: doesn't lowercase, so 'Cat' != 'cat'
    return [w for w in s.replace(',',' ').split() if w]
