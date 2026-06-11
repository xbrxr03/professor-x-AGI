class Accumulator:
    def __init__(self):
        self.total = 0
    def add(self, x):
        self.total = x
        return self.total
