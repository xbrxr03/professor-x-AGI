import merge

def resolve(defaults, overrides):
    return merge.deep(defaults, overrides)
