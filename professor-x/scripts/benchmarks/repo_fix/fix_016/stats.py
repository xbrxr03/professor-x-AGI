class RunningAverage:
    def __init__(self):
        self.total = 0
        self.count = 0

    def add(self, x):
        self.total += x
        # BUG: never increments self.count, so mean() divides by the wrong count.

    def mean(self):
        if self.count == 0:
            return 0
        return self.total / self.count
