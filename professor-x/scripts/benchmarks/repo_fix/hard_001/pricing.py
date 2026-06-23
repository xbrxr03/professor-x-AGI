import discount, tax

def compute(items, customer):
    subtotal = sum(p * q for p, q in items)
    after_disc = discount.apply(subtotal, customer)
    return round(tax.add(after_disc), 2)
