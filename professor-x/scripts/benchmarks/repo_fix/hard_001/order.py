import pricing

class Order:
    def __init__(self, items, customer):
        self.items = items
        self.customer = customer
    def total(self):
        return pricing.compute(self.items, self.customer)
