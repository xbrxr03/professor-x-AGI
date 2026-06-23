RATES = {"gold": 0.20, "silver": 0.10}

def apply(subtotal, customer):
    rate = RATES.get(customer, 0.0)
    return subtotal * rate
