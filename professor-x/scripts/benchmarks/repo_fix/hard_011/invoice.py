import money

def line_total(price, qty, tax_rate):
    return money.round_cents(price*qty*(1+tax_rate))
