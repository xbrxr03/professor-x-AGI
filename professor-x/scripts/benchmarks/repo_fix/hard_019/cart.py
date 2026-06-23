import promo

def final(price, codes):
    return round(promo.apply_all(price, codes), 2)
