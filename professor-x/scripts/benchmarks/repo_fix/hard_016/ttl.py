def fresh(age, limit):
    # BUG: should be expired when age > limit; uses >= so it expires one tick early
    return age < limit
