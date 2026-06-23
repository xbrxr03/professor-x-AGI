import basic

def variance(xs):
    m = basic.mean(xs)
    return sum((x - m) ** 2 for x in xs) / len(xs)
